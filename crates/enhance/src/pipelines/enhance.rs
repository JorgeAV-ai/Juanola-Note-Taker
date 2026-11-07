//! Enhance notes: notas del usuario + transcripción → `EnhancedNotes`.
//!
//! Con LFM3-1.7B, obligar a escribir JSON desde el primer token (gramática)
//! hunde la calidad: el modelo copia líneas del transcript en vez de resumir.
//! Por eso el modelo escribe Markdown libre (`## tema` y viñetas) y cita los
//! números de línea de la transcripción entre corchetes al final de cada viñeta.
//! Aquí se convierte a `EnhancedNotes`:
//!
//! - `[12, 14]` → `Citation::Transcript { start, end }` con los tiempos reales
//!   (el modelo no escribe timestamps: se los inventaría).
//! - Las notas del usuario se colocan de forma determinista, literales, al
//!   principio de la sección con la que más palabras comparten, con
//!   `is_user_text = true` y `Citation::UserNote`.
//! - Las viñetas de «Próximos pasos» son `is_action_item`.
//!
//! El resultado se valida contra [`crate::enhanced_notes_schema`].

use juanola_core_types::{Channel, Citation, Id, Segment};
use juanola_llm::{ChatModel, GenOptions, Message};

use crate::{enhanced_notes_schema, Bullet, EnhanceInput, EnhancedNotes, Section};

#[derive(Debug, thiserror::Error)]
pub enum EnhanceError {
    #[error("modelo: {0}")]
    Llm(#[from] juanola_llm::LlmError),
    #[error("el modelo no devolvió ninguna sección: {0}")]
    NoSections(String),
    #[error("la salida no cumple enhanced_notes_schema: {0}")]
    Schema(String),
    #[error("no hay transcripción ni notas")]
    Empty,
    #[error("la reunión es demasiado corta para generar notas ({words} palabras transcritas)")]
    TooShort { words: usize },
}

/// Bloque de las notas crudas del usuario.
#[derive(Debug, Clone)]
pub struct UserBlock {
    pub id: String,
    pub text: String,
}

/// Extrae los bloques de primer nivel de un documento ProseMirror. El id es
/// `attrs.id` si existe; si no, `b<n>` por posición.
pub fn user_blocks(doc: &serde_json::Value) -> Vec<UserBlock> {
    fn text_of(node: &serde_json::Value, out: &mut String) {
        if let Some(t) = node.get("text").and_then(|t| t.as_str()) {
            out.push_str(t);
        }
        if let Some(children) = node.get("content").and_then(|c| c.as_array()) {
            for c in children {
                text_of(c, out);
            }
        }
    }
    let Some(blocks) = doc.get("content").and_then(|c| c.as_array()) else { return Vec::new() };
    blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            let mut text = String::new();
            text_of(b, &mut text);
            let text = text.trim().to_owned();
            if text.is_empty() {
                return None;
            }
            let id = b
                .pointer("/attrs/id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("b{i}"));
            Some(UserBlock { id, text })
        })
        .collect()
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// Transcripción numerada desde 1, una línea por segmento final. Con `itn`,
/// los números escritos con letras pasan a dígitos.
pub fn format_transcript(segments: &[Segment], itn: bool) -> String {
    format_transcript_from(segments, 1, itn)
}

/// Como [`format_transcript`], numerando desde `first` (para fragmentos: la
/// numeración es la de la reunión completa, así las citas no cambian).
pub fn format_transcript_from(segments: &[Segment], first: usize, itn: bool) -> String {
    let mut out = String::new();
    for (i, s) in segments.iter().enumerate() {
        let who = match s.channel {
            Channel::Mic => "Yo",
            Channel::System => "Ellos",
        };
        let text = if itn { crate::itn::normalize_numbers(s.text.trim()) } else { s.text.trim().to_owned() };
        out.push_str(&format!("[{}] ({}) {who}: {text}\n", i + first, clock(s.start)));
    }
    out
}

/// Variantes del enhance (para evaluar; los valores por defecto son los buenos).
#[derive(Debug, Clone)]
pub struct EnhanceOptions {
    /// Normalizar números del transcript antes de dárselo al modelo.
    pub itn: bool,
    /// Plantilla del prompt; `None` = `juanola_prompts::ENHANCE_USER`.
    pub prompt: Option<String>,
    pub max_tokens: u32,
    /// Semilla del muestreo (para medir la varianza entre ejecuciones).
    pub seed: Option<u32>,
    /// Pasadas de detalle: tras el resumen, extraer datos concretos por
    /// fragmentos de esta duración y añadir los que falten a su sección.
    /// `None` = solo el resumen (3 veces más rápido, cobertura ~60 % frente a
    /// ~80 % en `fixtures/reunion_producto`).
    pub detail_chunk_secs: Option<f64>,
}

impl Default for EnhanceOptions {
    fn default() -> Self {
        Self { itn: true, prompt: None, max_tokens: 3000, seed: None, detail_chunk_secs: Some(240.0) }
    }
}

/// Divide los segmentos en rangos de `secs` segundos (por inicio de segmento).
/// Un último fragmento de menos del 40 % se une al anterior.
pub fn chunk_ranges(segments: &[Segment], secs: f64) -> Vec<std::ops::Range<usize>> {
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    let mut start = 0;
    while start < segments.len() {
        let limit = segments[start].start + secs;
        let mut end = start + 1;
        while end < segments.len() && segments[end].start < limit {
            end += 1;
        }
        out.push(start..end);
        start = end;
    }
    if out.len() >= 2 {
        let last = out.last().unwrap();
        let dur = segments[last.end - 1].end - segments[last.start].start;
        if dur < secs * 0.4 {
            let last = out.pop().unwrap();
            out.last_mut().unwrap().end = last.end;
        }
    }
    out
}

/// Prompt completo para el enhance (un único mensaje de usuario).
pub fn build_messages(input: &EnhanceInput<'_>, blocks: &[UserBlock], opts: &EnhanceOptions) -> Vec<Message> {
    let mut notes = String::new();
    if !blocks.is_empty() {
        notes.push_str("\nNotas que tomó el usuario durante la reunión:\n");
        for b in blocks {
            notes.push_str(&format!("- {}\n", b.text));
        }
    }
    let mut context = String::new();
    if !input.note.title.trim().is_empty() {
        context.push_str(&format!("Reunión: {}\n", input.note.title.trim()));
    }
    if !input.user_profile.trim().is_empty() {
        context.push_str(&format!("Sobre el usuario: {}\n", input.user_profile.trim()));
    }
    if let Some(t) = input.template {
        context.push_str(&format!(
            "Plantilla «{}». Propósito: {}. Estilo: {}. Estructura: {}.\n",
            t.name, t.purpose, t.style, t.structure
        ));
    }
    if !context.is_empty() {
        context.push('\n');
    }
    let template = opts.prompt.as_deref().unwrap_or(juanola_prompts::ENHANCE_USER);
    let transcript = format_transcript(input.segments, opts.itn);
    let user = template
        .replace("{{context}}", &context)
        .replace("{{transcript}}", transcript.trim_end())
        .replace("{{notes}}", &notes)
        .replace("{{sections}}", sections_hint(transcript_words(input.segments)))
        .replace("{{language}}", input.summary_language);
    vec![Message { role: "user".into(), content: user }]
}

/// Menos palabras que esto en la transcripción no da para resumir: el modelo
/// acaba repitiendo las instrucciones o inventando.
pub const MIN_WORDS: usize = 40;

pub fn transcript_words(segments: &[Segment]) -> usize {
    segments.iter().map(|s| s.text.split_whitespace().count()).sum()
}

/// Cuántos apartados pedir según lo que se habló.
fn sections_hint(words: usize) -> &'static str {
    match words {
        0..300 => "entre 1 y 3 apartados",
        300..1200 => "entre 2 y 6 apartados",
        _ => "entre 4 y 10 apartados",
    }
}

/// Tope de tokens del resumen según la longitud: en reuniones largas el
/// resumen ocupa ~0,4 tokens por palabra; con margen, y nunca más de `max`.
fn summary_budget(words: usize, max: u32) -> u32 {
    ((words as f32 * 0.8) as u32).clamp(300, max)
}

/// Viñeta tal como la escribe el modelo.
#[derive(Debug, Clone, PartialEq)]
struct RawBullet {
    text: String,
    refs: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq)]
struct RawSection {
    heading: String,
    bullets: Vec<RawBullet>,
}

/// Separa `texto [12, 14]` en texto y referencias. Recoge todos los grupos
/// de corchetes con números (también a mitad de frase, y rangos `12-14`) y
/// deja intactos los corchetes que no son citas.
fn split_refs(line: &str) -> (String, Vec<i64>) {
    fn parse_group(inner: &str) -> Option<Vec<i64>> {
        if inner.trim().is_empty() {
            return None;
        }
        let mut found = Vec::new();
        for part in inner.split([',', ';']) {
            let part = part.trim();
            if let Some((a, b)) = part.split_once('-') {
                let (a, b) = (a.trim().parse::<i64>().ok()?, b.trim().parse::<i64>().ok()?);
                if a > b || b - a > 10 {
                    return None;
                }
                found.extend(a..=b);
            } else {
                found.push(part.parse::<i64>().ok()?);
            }
        }
        Some(found)
    }
    let mut text = String::new();
    let mut refs = Vec::new();
    let mut rest = line.trim();
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']').map(|c| open + c) else { break };
        match parse_group(&rest[open + 1..close]) {
            Some(r) => {
                refs.extend(r);
                text.push_str(&rest[..open]);
            }
            None => text.push_str(&rest[..=close]),
        }
        rest = &rest[close + 1..];
    }
    text.push_str(rest);
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = text.replace(" ;", ";").replace(" ,", ",").replace(" .", ".");
    let text = text.trim().trim_end_matches([',', ';']).trim().to_owned();
    let text = text.strip_suffix('.').map(|t| format!("{t}.")).unwrap_or(text);
    (text, refs)
}

/// Título en cualquiera de las formas que usa el modelo: `#`…`####`, una línea
/// entera en negrita (`**Precios**`) o una línea corta que acaba en «:».
fn heading_of(l: &str) -> Option<String> {
    let hashes = l.chars().take_while(|c| *c == '#').count();
    if (1..=4).contains(&hashes) && l[hashes..].starts_with(' ') {
        return Some(l[hashes..].trim().to_owned());
    }
    if l.len() > 4 && l.starts_with("**") && l.ends_with("**") && !l[2..l.len() - 2].contains("**") {
        return Some(l[2..l.len() - 2].trim().trim_end_matches(':').to_owned());
    }
    if l.ends_with(':') && !l.starts_with(['-', '*', '•']) && l.split_whitespace().count() <= 6 && !l.contains('[') {
        return Some(l.trim_end_matches(':').to_owned());
    }
    None
}

/// Viñeta `- `, `* `, `• ` o numerada (`1. `, `2) `).
fn bullet_of(l: &str) -> Option<&str> {
    if let Some(b) = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")).or_else(|| l.strip_prefix("• ")) {
        return Some(b);
    }
    let digits = l.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits <= 2 {
        let rest = &l[digits..];
        return rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "));
    }
    None
}

fn clean_heading(h: &str) -> String {
    let h = h.trim().trim_matches(['*', '#', ' ']);
    let h = h.strip_prefix("Título:").or_else(|| h.strip_prefix("Tema:")).unwrap_or(h);
    h.trim().to_owned()
}

/// Markdown del modelo → secciones. Ignora el texto fuera de viñetas y las
/// secciones vacías; si un tema aparece dos veces, junta sus viñetas.
fn parse_markdown(md: &str) -> Vec<RawSection> {
    let mut sections: Vec<RawSection> = Vec::new();
    let mut current: Option<RawSection> = None;
    for line in md.lines() {
        let l = line.trim();
        if let Some(h) = heading_of(l) {
            if let Some(s) = current.take() {
                sections.push(s);
            }
            current = Some(RawSection { heading: clean_heading(&h), bullets: Vec::new() });
        } else if let Some(b) = bullet_of(l) {
            let (text, refs) = split_refs(b);
            let text = text.trim_matches('*').trim().to_owned();
            if text.is_empty() {
                continue;
            }
            current
                .get_or_insert_with(|| RawSection { heading: "Notas".into(), bullets: Vec::new() })
                .bullets
                .push(RawBullet { text, refs });
        }
    }
    if let Some(s) = current.take() {
        sections.push(s);
    }
    let mut merged: Vec<RawSection> = Vec::new();
    for s in sections.into_iter().filter(|s| !s.bullets.is_empty()) {
        match merged.iter_mut().find(|m| m.heading.to_lowercase() == s.heading.to_lowercase()) {
            Some(m) => m.bullets.extend(s.bullets),
            None => merged.push(s),
        }
    }
    merged
}

/// Palabras significativas de un título, para comparar temas.
fn heading_words(h: &str) -> std::collections::HashSet<String> {
    words(h).into_iter().filter(|w| !matches!(w.as_str(), "para" | "como" | "sobre" | "entre" | "desde")).collect()
}

/// Fusiona secciones del mismo tema (títulos con al menos la mitad de palabras
/// en común) en el orden de su primera aparición, quita viñetas repetidas y
/// junta todas las de tareas en un único «Próximos pasos» al final.
fn merge_sections(sections: Vec<RawSection>) -> Vec<RawSection> {
    let mut topics: Vec<RawSection> = Vec::new();
    let mut steps = RawSection { heading: "Próximos pasos".into(), bullets: Vec::new() };
    for s in sections {
        let target = if is_next_steps(&s.heading) {
            &mut steps
        } else {
            let hw = heading_words(&s.heading);
            let found = topics.iter().position(|t| {
                let tw = heading_words(&t.heading);
                let inter = hw.intersection(&tw).count();
                let uni = hw.union(&tw).count();
                t.heading.to_lowercase() == s.heading.to_lowercase() || (uni > 0 && inter * 2 >= uni)
            });
            match found {
                Some(i) => &mut topics[i],
                None => {
                    topics.push(RawSection { heading: s.heading.clone(), bullets: Vec::new() });
                    topics.last_mut().unwrap()
                }
            }
        };
        for b in s.bullets {
            if !target.bullets.iter().any(|x| x.text.to_lowercase() == b.text.to_lowercase()) {
                target.bullets.push(b);
            }
        }
    }
    if !steps.bullets.is_empty() {
        topics.push(steps);
    }
    topics
}

/// Palabras de 4+ letras y números de un texto, para detectar duplicados.
fn key_terms(s: &str) -> std::collections::HashSet<String> {
    words(&crate::itn::normalize_numbers(s))
}

/// Un «dato concreto» tiene alguna cifra o algún nombre propio (mayúscula que
/// no abre la frase); lo demás de la pasada de detalle suele ser ruido
/// («Prefiero eso a perderlas»). Tampoco valen las líneas copiadas con su
/// hablante («Yo: …»).
fn is_concrete(text: &str) -> bool {
    let t = text.trim();
    if t.starts_with("Yo:") || t.starts_with("Ellos:") {
        return false;
    }
    if t.chars().any(|c| c.is_ascii_digit()) {
        return true;
    }
    t.split_whitespace()
        .skip(1)
        .any(|w| w.chars().next().is_some_and(char::is_uppercase) && w.chars().filter(|c| c.is_alphabetic()).count() >= 3)
}

/// Máximo de datos añadidos por sección en la pasada de detalle.
const MAX_DETAILS_PER_SECTION: usize = 4;

/// Añade a `sections` los datos de la pasada de detalle que no estén ya:
/// cada uno va a la sección (que no sea de tareas) cuyas citas están más
/// cerca de las suyas. Se descartan los que no citan nada válido y los que se
/// solapan con una viñeta existente (citas cercanas y palabras en común).
fn add_details(sections: &mut [RawSection], details: Vec<RawBullet>, n_lines: usize) {
    let valid = |r: &i64| *r >= 1 && (*r as usize) <= n_lines;
    let mut added = vec![0usize; sections.len()];
    for d in details.into_iter().filter(|d| is_concrete(&d.text)) {
        let refs: Vec<i64> = d.refs.iter().copied().filter(valid).collect();
        let Some(&anchor) = refs.first() else { continue };
        let terms = key_terms(&d.text);
        if terms.is_empty() {
            continue;
        }
        // ¿Ya está? Una viñeta con citas a ≤ 2 líneas y la mitad de términos en común,
        // o que ya contiene todos sus números.
        let duplicate = sections.iter().flat_map(|s| &s.bullets).any(|b| {
            let near = b.refs.iter().any(|r| refs.iter().any(|x| (r - x).abs() <= 2));
            if !near {
                return false;
            }
            let bt = key_terms(&b.text);
            let common = terms.intersection(&bt).count();
            let nums: Vec<&String> = terms.iter().filter(|t| t.chars().all(|c| c.is_ascii_digit())).collect();
            common * 2 >= terms.len() || (!nums.is_empty() && nums.iter().all(|n| bt.contains(*n)))
        });
        if duplicate {
            continue;
        }
        let target = sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !is_next_steps(&s.heading))
            .filter_map(|(i, s)| {
                s.bullets.iter().flat_map(|b| b.refs.iter()).filter(|r| valid(r)).map(|r| (r - anchor).abs()).min().map(|dist| (i, dist))
            })
            .min_by_key(|(_, dist)| *dist);
        let Some((i, dist)) = target else { continue };
        if dist > 6 || added[i] >= MAX_DETAILS_PER_SECTION {
            continue;
        }
        added[i] += 1;
        // Insertar tras la última viñeta que cita una línea anterior.
        let pos = sections[i].bullets.iter().rposition(|b| b.refs.iter().any(|r| valid(r) && *r <= anchor)).map_or(0, |p| p + 1);
        sections[i].bullets.insert(pos, RawBullet { text: d.text, refs });
    }
}

fn is_next_steps(heading: &str) -> bool {
    let h = heading.to_lowercase();
    ["próximos pasos", "proximos pasos", "tareas", "next steps", "action items"].iter().any(|k| h.contains(k))
}

/// Palabras de 4+ letras en minúsculas (para emparejar notas con secciones).
fn words(s: &str) -> std::collections::HashSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4 || w.chars().all(|c| c.is_ascii_digit()) && !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Métricas de calidad de una salida (para pruebas y telemetría local).
#[derive(Debug, Clone, Default)]
pub struct EnhanceReport {
    pub sections: usize,
    pub bullets: usize,
    pub user_bullets: usize,
    pub action_items: usize,
    /// Viñetas de la IA sin ninguna referencia válida a la transcripción.
    pub uncited: usize,
    /// Números de línea fuera de rango.
    pub invalid_refs: usize,
}

fn resolve(
    raw: Vec<RawSection>,
    segments: &[Segment],
    blocks: &[UserBlock],
    note_id: Id,
) -> Result<(EnhancedNotes, EnhanceReport), EnhanceError> {
    let mut report = EnhanceReport::default();
    let mut sections: Vec<Section> = raw
        .into_iter()
        .map(|rs| {
            let action = is_next_steps(&rs.heading);
            let bullets = rs
                .bullets
                .into_iter()
                .filter_map(|rb| {
                    let mut span: Option<(f64, f64)> = None;
                    for r in &rb.refs {
                        match usize::try_from(*r).ok().filter(|&r| r >= 1 && r <= segments.len()) {
                            Some(i) => {
                                let s = &segments[i - 1];
                                span = Some(match span {
                                    Some((a, b)) => (a.min(s.start), b.max(s.end)),
                                    None => (s.start, s.end),
                                });
                            }
                            None => report.invalid_refs += 1,
                        }
                    }
                    // Lo que la IA no puede respaldar con la transcripción no se
                    // muestra: suele ser invento o eco de las instrucciones.
                    let Some((start, end)) = span else {
                        report.uncited += 1;
                        return None;
                    };
                    Some(Bullet {
                        text: rb.text,
                        is_user_text: false,
                        source: Some(Citation::Transcript { note_id, start, end }),
                        is_action_item: action,
                    })
                })
                .collect();
            Section { heading: rs.heading, bullets }
        })
        .filter(|s: &Section| !s.bullets.is_empty())
        .collect();
    if sections.is_empty() && blocks.is_empty() {
        return Err(EnhanceError::NoSections("ninguna viñeta cita la transcripción".into()));
    }

    // Notas del usuario: al principio de la sección (que no sea de tareas) con
    // la que más palabras comparten; si ninguna coincide, a una sección propia.
    let mut orphans = Vec::new();
    let mut placed: Vec<Vec<Bullet>> = vec![Vec::new(); sections.len()];
    for b in blocks {
        let bw = words(&b.text);
        let best = sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !is_next_steps(&s.heading))
            .map(|(i, s)| {
                let sw: std::collections::HashSet<String> =
                    std::iter::once(s.heading.as_str()).chain(s.bullets.iter().map(|b| b.text.as_str())).flat_map(words).collect();
                (i, bw.intersection(&sw).count())
            })
            .filter(|(_, n)| *n > 0)
            .max_by_key(|(i, n)| (*n, std::cmp::Reverse(*i)));
        let bullet = Bullet {
            text: b.text.clone(),
            is_user_text: true,
            source: Some(Citation::UserNote { note_id, block_id: b.id.clone() }),
            is_action_item: false,
        };
        match best {
            Some((i, _)) => placed[i].push(bullet),
            None => orphans.push(bullet),
        }
    }
    for (s, mut user) in sections.iter_mut().zip(placed) {
        report.user_bullets += user.len();
        user.append(&mut s.bullets);
        s.bullets = user;
    }
    if !orphans.is_empty() {
        report.user_bullets += orphans.len();
        sections.insert(0, Section { heading: "Tus notas".into(), bullets: orphans });
    }

    report.sections = sections.len();
    report.bullets = sections.iter().map(|s| s.bullets.len()).sum();
    report.action_items = sections.iter().flat_map(|s| &s.bullets).filter(|b| b.is_action_item).count();
    let notes = EnhancedNotes { sections };
    validate(&notes)?;
    Ok((notes, report))
}

/// Valida contra el esquema público de las notas mejoradas.
pub fn validate(notes: &EnhancedNotes) -> Result<(), EnhanceError> {
    let value = serde_json::to_value(notes).map_err(|e| EnhanceError::Schema(e.to_string()))?;
    let validator = jsonschema::validator_for(&enhanced_notes_schema()).map_err(|e| EnhanceError::Schema(e.to_string()))?;
    let errors: Vec<String> = validator.iter_errors(&value).map(|e| format!("{} en {}", e, e.instance_path())).collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(EnhanceError::Schema(errors.join("; ")))
    }
}

/// Ejecuta el enhance completo con las opciones por defecto.
pub async fn enhance(model: &dyn ChatModel, input: &EnhanceInput<'_>) -> Result<(EnhancedNotes, EnhanceReport), EnhanceError> {
    enhance_with(model, input, &EnhanceOptions::default()).await
}

/// Ejecuta el enhance con opciones explícitas.
pub async fn enhance_with(
    model: &dyn ChatModel,
    input: &EnhanceInput<'_>,
    opts: &EnhanceOptions,
) -> Result<(EnhancedNotes, EnhanceReport), EnhanceError> {
    let blocks = user_blocks(&input.note.raw_doc);
    if input.segments.is_empty() && blocks.is_empty() {
        return Err(EnhanceError::Empty);
    }
    let words = transcript_words(input.segments);
    if words < MIN_WORDS {
        // Sin conversación suficiente: solo las notas del usuario, sin modelo.
        if blocks.is_empty() {
            return Err(EnhanceError::TooShort { words });
        }
        return resolve(Vec::new(), input.segments, &blocks, input.note.id);
    }
    let gen = GenOptions { max_tokens: Some(summary_budget(words, opts.max_tokens)), seed: opts.seed, ..Default::default() };
    // 1. Resumen en una pasada: da la estructura y las tareas.
    let md = model.complete(&build_messages(input, &blocks, opts), &gen).await?;
    let mut raw = merge_sections(parse_markdown(&md));
    // 2. Pasadas de detalle por fragmentos: datos concretos que el resumen omitió.
    // (Resumir por fragmentos y reagrupar los temas con el modelo se probó y
    // daba peor estructura y perdía tareas.)
    if let (Some(secs), false) = (opts.detail_chunk_secs, raw.is_empty()) {
        for range in chunk_ranges(input.segments, secs) {
            let segs = &input.segments[range.clone()];
            let prompt = juanola_prompts::ENHANCE_DETAILS
                .replace("{{from}}", &clock(segs[0].start))
                .replace("{{to}}", &clock(segs[segs.len() - 1].end))
                .replace("{{transcript}}", format_transcript_from(segs, range.start + 1, opts.itn).trim_end())
                .replace("{{language}}", input.summary_language);
            // 12 datos caben en ~450 tokens; el tope corta las listas que el
            // modelo alarga sin fin (es lo que más tiempo cuesta).
            let detail_gen = GenOptions { max_tokens: Some(450), ..gen.clone() };
            let md = model.complete(&[Message { role: "user".into(), content: prompt }], &detail_gen).await?;
            let details: Vec<RawBullet> = parse_markdown(&md).into_iter().flat_map(|s| s.bullets).collect();
            add_details(&mut raw, details, input.segments.len());
        }
    }
    if raw.is_empty() {
        return Err(EnhanceError::NoSections(md));
    }
    // (Se probó una pasada aparte para las tareas en JSON con gramática: daba
    // peores tareas, con frases copiadas y sin responsable.)
    resolve(raw, input.segments, &blocks, input.note.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(i: usize, ch: Channel, start: f64, end: f64, text: &str) -> Segment {
        Segment {
            id: Id::from_u128(i as u128),
            note_id: Id::nil(),
            channel: ch,
            speaker_id: None,
            start,
            end,
            text: text.into(),
            confidence: None,
            is_final: true,
        }
    }

    #[test]
    fn blocks_from_prosemirror() {
        let doc = serde_json::json!({"type":"doc","content":[
            {"type":"paragraph","content":[{"type":"text","text":"presupuesto Q3"}]},
            {"type":"paragraph"},
            {"type":"bulletList","attrs":{"id":"x9"},"content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"demo jueves"}]}]}]}
        ]});
        let b = user_blocks(&doc);
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].id.as_str(), b[0].text.as_str()), ("b0", "presupuesto Q3"));
        assert_eq!((b[1].id.as_str(), b[1].text.as_str()), ("x9", "demo jueves"));
    }

    #[test]
    fn transcript_is_numbered_from_one() {
        let s = [seg(0, Channel::Mic, 65.0, 70.0, " hola "), seg(1, Channel::System, 71.0, 72.0, "qué tal")];
        assert_eq!(format_transcript(&s, false), "[1] (01:05) Yo: hola\n[2] (01:11) Ellos: qué tal\n");
        let n = [seg(0, Channel::System, 0.0, 1.0, "son treinta y nueve euros")];
        assert_eq!(format_transcript(&n, true), "[1] (00:00) Ellos: son 39 euros\n");
    }

    #[test]
    fn refs_are_split_from_text() {
        assert_eq!(split_refs("Ingresos de 112 mil euros [7]"), ("Ingresos de 112 mil euros".into(), vec![7]));
        assert_eq!(split_refs("Algo [3, 5] [8-10]."), ("Algo.".into(), vec![3, 5, 8, 9, 10]));
        assert_eq!(split_refs("PDF falla con tildes [24]; se arregla esta semana."), ("PDF falla con tildes; se arregla esta semana.".into(), vec![24]));
        assert_eq!(split_refs("Lista [a, b]"), ("Lista [a, b]".into(), vec![]));
        assert_eq!(split_refs("Sin citas"), ("Sin citas".into(), vec![]));
    }

    #[test]
    fn markdown_sections_are_parsed_and_merged() {
        let md = "Aquí tienes:\n## Título: Precios\n- Planes en enero [2]\n\n## Errores\n- Citas duplicadas [1, 3]\n## precios\n* Latam caro [2]\n## Vacío\n";
        let s = parse_markdown(md);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].heading, "Precios");
        assert_eq!(s[0].bullets.len(), 2);
        assert_eq!(s[1].bullets[0], RawBullet { text: "Citas duplicadas".into(), refs: vec![1, 3] });
    }

    struct NoModel;

    #[async_trait::async_trait]
    impl ChatModel for NoModel {
        fn context_window(&self) -> usize {
            0
        }
        async fn complete(&self, _: &[Message], _: &GenOptions) -> Result<String, juanola_llm::LlmError> {
            panic!("no se debe llamar al modelo con una transcripción tan corta");
        }
        fn stream<'a>(&'a self, _: &'a [Message], _: &'a GenOptions) -> futures::stream::BoxStream<'a, Result<String, juanola_llm::LlmError>> {
            unreachable!()
        }
    }

    fn note(raw_doc: serde_json::Value) -> juanola_core_types::Note {
        juanola_core_types::Note {
            id: Id::nil(),
            workspace_id: Id::nil(),
            title: String::new(),
            raw_doc,
            enhanced_doc: None,
            template_id: None,
            status: juanola_core_types::NoteStatus::Draft,
            calendar_event_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            trashed_at: None,
        }
    }

    #[tokio::test]
    async fn short_meetings_skip_the_model() {
        let segs = [seg(0, Channel::Mic, 1.0, 2.5, "Para entonces como ya"), seg(1, Channel::Mic, 9.0, 10.0, "Pero bueno")];
        let empty = note(serde_json::json!({"type": "doc", "content": []}));
        let input = EnhanceInput { note: &empty, segments: &segs, template: None, summary_language: "español", user_profile: "" };
        assert!(matches!(enhance(&NoModel, &input).await, Err(EnhanceError::TooShort { words: 6 })));
        // Con notas del usuario: se devuelven tal cual, sin modelo.
        let with_notes = note(serde_json::json!({"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "llamar a Luis"}]}]}));
        let input = EnhanceInput { note: &with_notes, segments: &segs, template: None, summary_language: "español", user_profile: "" };
        let (notes, _) = enhance(&NoModel, &input).await.unwrap();
        assert_eq!(notes.sections[0].bullets[0].text, "llamar a Luis");
    }

    #[test]
    fn budget_and_sections_scale_with_length() {
        assert_eq!(sections_hint(20), "entre 1 y 3 apartados");
        assert_eq!(sections_hint(2400), "entre 4 y 10 apartados");
        assert_eq!(summary_budget(30, 3000), 300);
        assert_eq!(summary_budget(2400, 3000), 1920);
        assert_eq!(summary_budget(10_000, 3000), 3000);
    }

    #[test]
    fn chunks_split_by_time_and_absorb_short_tail() {
        let segs: Vec<Segment> = (0..10).map(|i| seg(i, Channel::Mic, i as f64 * 60.0, i as f64 * 60.0 + 50.0, "x")).collect();
        assert_eq!(chunk_ranges(&segs, 300.0), vec![0..5, 5..10]);
        // 11 minutos: el último minuto se une al fragmento anterior.
        let segs: Vec<Segment> = (0..11).map(|i| seg(i, Channel::Mic, i as f64 * 60.0, i as f64 * 60.0 + 50.0, "x")).collect();
        assert_eq!(chunk_ranges(&segs, 300.0), vec![0..5, 5..11]);
    }

    #[test]
    fn details_fill_gaps_without_duplicates() {
        let mut secs = parse_markdown("## Métricas\n- MRR de 112000 euros [7]\n## Precios\n- Tres planes nuevos [30]\n## Próximos pasos\n- Luis llama [50]\n");
        let details = parse_markdown(
            "- Ingresos recurrentes de 112000 euros [7]\n- 1840 clínicas activas [6]\n- Plan básico de 39 euros [31]\n- Sin cita\n- Algo lejano [90]\n",
        )
        .into_iter()
        .flat_map(|s| s.bullets)
        .collect();
        add_details(&mut secs, details, 100);
        let m: Vec<&str> = secs[0].bullets.iter().map(|b| b.text.as_str()).collect();
        assert_eq!(m, ["1840 clínicas activas", "MRR de 112000 euros"], "el duplicado del MRR no entra; el nuevo va en orden");
        assert_eq!(secs[1].bullets.len(), 2);
        assert_eq!(secs[2].bullets.len(), 1, "las tareas no reciben detalles");
    }

    #[test]
    fn only_concrete_details() {
        assert!(is_concrete("Plan básico de 39 euros"));
        assert!(is_concrete("La auditoría la hará Elena"));
        assert!(!is_concrete("Prefiero eso a perderlas."));
        assert!(!is_concrete("Yo: 3 cosas"));
    }

    #[test]
    fn headings_in_other_formats() {
        let s = parse_markdown("# Métricas\n1. MRR 112000 [3]\n**Precios**\n2) básico 39 [5]\nContratación:\n- Elena [7]\n");
        let h: Vec<&str> = s.iter().map(|x| x.heading.as_str()).collect();
        assert_eq!(h, ["Métricas", "Precios", "Contratación"]);
        assert_eq!(s[1].bullets[0].refs, vec![5]);
    }

    #[test]
    fn similar_topics_merge_and_tasks_go_last() {
        let a = parse_markdown("## Planes de precios\n- a [1]\n## Próximos pasos\n- t1 [2]\n");
        let b = parse_markdown("## Precios y planes\n- b [3]\n- a [1]\n## Contratación\n- c [4]\n## Próximos pasos\n- t2 [5]\n");
        let m = merge_sections(a.into_iter().chain(b).collect());
        let heads: Vec<&str> = m.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(heads, ["Planes de precios", "Contratación", "Próximos pasos"]);
        assert_eq!(m[0].bullets.len(), 2, "«a» repetida se quita");
        assert_eq!(m[2].bullets.len(), 2);
    }

    #[test]
    fn resolve_maps_refs_places_user_notes_and_marks_tasks() {
        let segs = [
            seg(0, Channel::Mic, 1.0, 2.0, "a"),
            seg(1, Channel::System, 3.0, 5.0, "b"),
            seg(2, Channel::System, 6.0, 9.0, "c"),
        ];
        let blocks = [
            UserBlock { id: "b0".into(), text: "precios nuevos: enero".into() },
            UserBlock { id: "b1".into(), text: "llamar a la gestoría".into() },
        ];
        let raw = parse_markdown("## Errores\n- Citas duplicadas [3, 2, 99]\n## Precios\n- Los precios nuevos entran en enero [1]\n- Sin cita\n## Inventado\n- Nada que lo respalde\n## Próximos pasos\n- Luis llama hoy [2]\n");
        let (notes, rep) = resolve(raw, &segs, &blocks, Id::nil()).unwrap();
        assert_eq!(notes.sections[0].heading, "Tus notas", "la nota sin coincidencias va a su sección");
        let precios = notes.sections.iter().find(|s| s.heading == "Precios").unwrap();
        assert!(precios.bullets[0].is_user_text && precios.bullets[0].text == "precios nuevos: enero");
        let errores = notes.sections.iter().find(|s| s.heading == "Errores").unwrap();
        match &errores.bullets[0].source {
            Some(Citation::Transcript { start, end, .. }) => assert_eq!((*start, *end), (3.0, 9.0)),
            other => panic!("{other:?}"),
        }
        assert!(notes.sections.last().unwrap().bullets[0].is_action_item);
        assert_eq!((rep.uncited, rep.invalid_refs, rep.action_items, rep.user_bullets), (2, 1, 1, 2));
        assert!(notes.sections.iter().all(|s| s.heading != "Inventado"), "las secciones sin citas desaparecen");
        let precios = notes.sections.iter().find(|s| s.heading == "Precios").unwrap();
        assert!(precios.bullets.iter().all(|b| b.text != "Sin cita"));
    }
}
