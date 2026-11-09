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
//! Con plantilla, el modelo sigue escribiendo Markdown y [`fit_to_template`]
//! encaja su salida en los apartados de la plantilla: los títulos finales son
//! siempre los de la plantilla, en su orden, escriba lo que escriba el modelo.
//! Si no encaja, se reintenta una vez con el motivo y, si vuelve a fallar, se
//! genera en Auto (`EnhanceReport::template_fallback`).
//!
//! El resultado se valida contra [`crate::enhanced_notes_schema`].

use juanola_core_types::{fold, Channel, Citation, Id, SectionKind, Segment, Template, TemplateSection};
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

/// Qué es cada bloque de las notas crudas del usuario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading(u8),
    /// Elemento de una lista con viñetas o numerada.
    ListItem,
    /// Elemento de una lista de tareas (`[ ]` en el editor).
    Task { checked: bool },
}

/// Bloque de las notas crudas del usuario: un párrafo, un título o un
/// elemento de lista (cada elemento por separado, también los anidados).
#[derive(Debug, Clone)]
pub struct UserBlock {
    pub id: String,
    pub text: String,
    pub kind: BlockKind,
    /// Alguna parte en negrita o subrayada: el usuario lo marcó como importante.
    pub emphasis: bool,
}

/// Extrae los bloques de texto de un documento ProseMirror (TipTap). Los
/// párrafos y títulos de primer nivel son un bloque cada uno; las listas se
/// abren y cada elemento es un bloque, con sus sublistas detrás. Las imágenes
/// y separadores se ignoran.
///
/// El id es `attrs.id` si existe; si no, la posición: `b<n>` en el primer
/// nivel y `b<n>.<m>` (y así sucesivamente) dentro de listas y citas.
pub fn user_blocks(doc: &serde_json::Value) -> Vec<UserBlock> {
    /// Texto en línea de un nodo (párrafo, título): los nodos de texto
    /// hermanos ya traen sus espacios; los saltos de línea pasan a espacio.
    fn inline(node: &serde_json::Value, text: &mut String, emphasis: &mut bool) {
        match node.get("type").and_then(|t| t.as_str()) {
            Some("text") => {
                if let Some(t) = node.get("text").and_then(|t| t.as_str()) {
                    text.push_str(t);
                }
                let marks = node.get("marks").and_then(|m| m.as_array()).into_iter().flatten();
                if marks.filter_map(|m| m.get("type").and_then(|t| t.as_str())).any(|t| matches!(t, "bold" | "underline")) {
                    *emphasis = true;
                }
            }
            Some("hardBreak") => text.push(' '),
            _ => {
                for c in children(node) {
                    inline(c, text, emphasis);
                }
            }
        }
    }
    fn children(node: &serde_json::Value) -> impl Iterator<Item = &serde_json::Value> {
        node.get("content").and_then(|c| c.as_array()).into_iter().flatten()
    }
    fn push(out: &mut Vec<UserBlock>, node: &serde_json::Value, id: String, kind: BlockKind, text: String, emphasis: bool) {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            return;
        }
        let id = node.pointer("/attrs/id").and_then(|v| v.as_str()).map(str::to_owned).unwrap_or(id);
        out.push(UserBlock { id, text, kind, emphasis });
    }
    fn walk(node: &serde_json::Value, id: String, out: &mut Vec<UserBlock>) {
        let ty = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ty {
            "paragraph" | "heading" | "codeBlock" => {
                let kind = match ty {
                    "heading" => BlockKind::Heading(node.pointer("/attrs/level").and_then(|l| l.as_u64()).unwrap_or(1).clamp(1, 6) as u8),
                    _ => BlockKind::Paragraph,
                };
                let (mut text, mut emphasis) = (String::new(), false);
                inline(node, &mut text, &mut emphasis);
                push(out, node, id, kind, text, emphasis);
            }
            "listItem" | "taskItem" => {
                let kind = if ty == "taskItem" {
                    BlockKind::Task { checked: node.pointer("/attrs/checked").and_then(|c| c.as_bool()).unwrap_or(false) }
                } else {
                    BlockKind::ListItem
                };
                // El texto del elemento son sus párrafos; las sublistas van
                // detrás como bloques propios.
                let (mut text, mut emphasis) = (String::new(), false);
                let mut nested = Vec::new();
                for c in children(node) {
                    if matches!(c.get("type").and_then(|t| t.as_str()), Some("paragraph" | "heading" | "codeBlock")) {
                        text.push(' ');
                        inline(c, &mut text, &mut emphasis);
                    } else {
                        nested.push(c);
                    }
                }
                push(out, node, id.clone(), kind, text, emphasis);
                let mut n = 0;
                for c in nested {
                    for item in children(c) {
                        walk(item, format!("{id}.{n}"), out);
                        n += 1;
                    }
                }
            }
            "image" | "horizontalRule" => {}
            // Listas, citas y cualquier contenedor desconocido: sus hijos.
            _ => {
                for (n, c) in children(node).enumerate() {
                    walk(c, format!("{id}.{n}"), out);
                }
            }
        }
    }
    let mut out = Vec::new();
    for (i, b) in children(doc).enumerate() {
        walk(b, format!("b{i}"), &mut out);
    }
    out
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
    /// Con plantilla: gramática GBNF sobre Markdown que fija los títulos de la
    /// plantilla, en su orden, y deja libre el texto de las viñetas (cada una
    /// acaba en sus citas `[n, m]`). Experimental: ver `docs/plantillas.md` §6.4.
    pub markdown_grammar: bool,
}

impl Default for EnhanceOptions {
    fn default() -> Self {
        Self { itn: true, prompt: None, max_tokens: 3000, seed: None, detail_chunk_secs: Some(240.0), markdown_grammar: false }
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

/// Las notas del usuario tal como van al prompt: los títulos se mantienen como
/// títulos, las tareas llevan `[tarea]` y lo marcado en negrita o subrayado
/// (o escrito bajo un título `###`) lleva `[importante]`.
fn format_user_notes(blocks: &[UserBlock]) -> String {
    if blocks.is_empty() {
        return String::new();
    }
    let mut notes = String::from(
        "\nNotas que tomó el usuario durante la reunión (marcan lo que le importó; \
         desarrolla esos temas con lo que se dijo en la transcripción, sin copiarlas):\n",
    );
    let mut under_h3 = false;
    for b in blocks {
        match b.kind {
            BlockKind::Heading(level) => {
                under_h3 = level == 3;
                notes.push_str(&format!("## {}\n", b.text));
            }
            BlockKind::Task { .. } => notes.push_str(&format!("- [tarea] {}\n", b.text)),
            _ if b.emphasis || under_h3 => notes.push_str(&format!("- [importante] {}\n", b.text)),
            _ => notes.push_str(&format!("- {}\n", b.text)),
        }
    }
    notes
}

fn context_block(input: &EnhanceInput<'_>, template: Option<&Template>) -> String {
    let mut context = String::new();
    if !input.note.title.trim().is_empty() {
        context.push_str(&format!("Reunión: {}\n", input.note.title.trim()));
    }
    if !input.user_profile.trim().is_empty() {
        context.push_str(&format!("Sobre el usuario: {}\n", input.user_profile.trim()));
    }
    if let Some(t) = template {
        let mut line = format!("Plantilla «{}».", t.name.trim());
        if !t.purpose.trim().is_empty() {
            line.push_str(&format!(" Propósito: {}.", t.purpose.trim().trim_end_matches('.')));
        }
        if !t.style.trim().is_empty() {
            line.push_str(&format!(" Estilo: {}.", t.style.trim().trim_end_matches('.')));
        }
        context.push_str(&line);
        context.push('\n');
    }
    if !context.is_empty() {
        context.push('\n');
    }
    context
}

/// Descripción de un apartado para el prompt (`## título — descripción`).
fn structure_lines(sections: &[TemplateSection]) -> String {
    sections
        .iter()
        .map(|s| {
            let hint = match (s.hint.trim(), s.kind) {
                ("", SectionKind::Tasks) => "cada tarea: quién, qué y cuándo",
                (h, _) => h,
            };
            if hint.is_empty() {
                format!("## {}", s.heading.trim())
            } else {
                format!("## {} — {hint}", s.heading.trim())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Gramática GBNF del resumen con plantilla: los títulos de la plantilla en su
/// orden (y «Otros temas» opcional al final), cada uno con 0 a 8 viñetas que
/// acaban en sus citas. El texto de las viñetas es libre: lo que hundía la
/// calidad era forzar JSON, no la estructura.
fn markdown_grammar(sections: &[TemplateSection], language: &str) -> String {
    fn lit(s: &str) -> String {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }
    let mut g = String::from("root ::= ");
    let names: Vec<String> = (0..sections.len()).map(|i| format!("s{i}")).collect();
    g.push_str(&names.join(" "));
    let other = !sections.iter().any(|s| crate::language::is_other_topics(&s.heading));
    if other {
        g.push_str(" other?");
    }
    g.push('\n');
    for (i, s) in sections.iter().enumerate() {
        g.push_str(&format!("s{i} ::= {} \"\\n\" body\n", lit(&format!("## {}", s.heading.trim()))));
    }
    if other {
        g.push_str(&format!(
            "other ::= {} \"\\n\" body\n",
            lit(&format!("## {}", crate::language::localize_heading(OTHER_TOPICS, language)))
        ));
    }
    g.push_str(concat!(
        "body ::= \"\\n\"? bullet{0,8} \"\\n\"?\n",
        "bullet ::= \"- \" text refs \"\\n\"\n",
        "text ::= [^\\n\\[]{3,300}\n",
        "refs ::= \"[\" [0-9]{1,4} (\", \" [0-9]{1,4}){0,9} \"]\"\n",
    ));
    g
}

/// Los apartados de la plantilla más un `## Otros temas` opcional: sin esa
/// salida, el modelo mete en los apartados lo que no les corresponde (medido:
/// con una plantilla que no encaja, rellena todos y copia frases literales).
fn structure_with_outlet(sections: &[TemplateSection], language: &str) -> String {
    let mut lines = structure_lines(sections);
    if !sections.iter().any(|s| crate::language::is_other_topics(&s.heading)) {
        lines.push_str(&format!(
            "\n## {} — solo si hace falta: lo que se habló y no corresponde a ninguno de los apartados anteriores",
            crate::language::localize_heading(OTHER_TOPICS, language)
        ));
    }
    lines
}

/// Prompt completo para el enhance (un único mensaje de usuario). Con
/// plantilla en `input.template` usa [`juanola_prompts::ENHANCE_TEMPLATE`].
pub fn build_messages(input: &EnhanceInput<'_>, blocks: &[UserBlock], opts: &EnhanceOptions) -> Vec<Message> {
    match input.template {
        Some(t) => template_messages(input, t, blocks, opts, None),
        None => auto_messages(input, blocks, opts),
    }
}

fn auto_messages(input: &EnhanceInput<'_>, blocks: &[UserBlock], opts: &EnhanceOptions) -> Vec<Message> {
    let template = opts.prompt.as_deref().unwrap_or(juanola_prompts::ENHANCE_USER);
    let transcript = format_transcript(input.segments, opts.itn);
    let user = template
        .replace("{{context}}", &context_block(input, None))
        .replace("{{transcript}}", transcript.trim_end())
        .replace("{{notes}}", &format_user_notes(blocks))
        .replace("{{sections}}", sections_hint(transcript_words(input.segments)))
        .replace("{{language}}", input.summary_language)
        + &crate::language::prompt_suffix(input.summary_language);
    vec![Message { role: "user".into(), content: user }]
}

/// Prompt con plantilla; `retry` es el motivo del fallo del intento anterior,
/// que va al final (lo último que lee el modelo).
fn template_messages(
    input: &EnhanceInput<'_>,
    template: &Template,
    blocks: &[UserBlock],
    opts: &EnhanceOptions,
    retry: Option<&str>,
) -> Vec<Message> {
    let transcript = format_transcript(input.segments, opts.itn);
    let mut user = juanola_prompts::ENHANCE_TEMPLATE
        .replace("{{context}}", &context_block(input, Some(template)))
        .replace("{{transcript}}", transcript.trim_end())
        .replace("{{notes}}", &format_user_notes(blocks))
        .replace("{{structure}}", &structure_with_outlet(&template.structure, input.summary_language))
        .replace("{{language}}", input.summary_language)
        + &crate::language::template_prompt_suffix(input.summary_language);
    if let Some(r) = retry {
        user.push('\n');
        user.push_str(r);
    }
    vec![Message { role: "user".into(), content: user.trim_end().to_owned() + "\n" }]
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

/// Tope de tokens con plantilla: el de Auto más ~150 por apartado (incluido
/// «Otros temas»), con un mínimo de 800.
fn template_budget(words: usize, sections: usize, max: u32) -> u32 {
    (summary_budget(words, max) + 150 * (sections as u32 + 1)).clamp(800, max.max(800))
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
            // Sin negritas: el editor muestra el texto tal cual.
            let text = text.replace("**", "").trim_matches('*').trim().to_owned();
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

/// ¿El título `a` (del modelo o del usuario) es el apartado `b` de la
/// plantilla? Por orden de confianza: iguales sin mayúsculas, tildes ni
/// puntuación (3); uno contiene al otro como palabras enteras (2); al menos la
/// mitad de palabras significativas en común (1).
fn heading_match(a: &str, b: &str) -> Option<u8> {
    let (fa, fb) = (fold(a), fold(b));
    if fa.is_empty() || fb.is_empty() {
        return None;
    }
    if fa == fb {
        return Some(3);
    }
    let (pa, pb) = (format!(" {fa} "), format!(" {fb} "));
    if pa.contains(&pb) || pb.contains(&pa) {
        return Some(2);
    }
    let (wa, wb) = (heading_words(&fa), heading_words(&fb));
    let (inter, uni) = (wa.intersection(&wb).count(), wa.union(&wb).count());
    (inter > 0 && inter * 2 >= uni).then_some(1)
}

/// Apartado de `headings` que mejor encaja con `heading` por título (pasos
/// 1.1 a 1.3 del plan); a igual confianza, el primero.
fn best_heading(heading: &str, headings: &[&str]) -> Option<usize> {
    headings
        .iter()
        .enumerate()
        .filter_map(|(i, h)| heading_match(heading, h).map(|q| (i, q)))
        .max_by_key(|(i, q)| (*q, std::cmp::Reverse(*i)))
        .map(|(i, _)| i)
}

/// Palabras para colocar viñetas y notas: sin tildes y con las cifras
/// normalizadas («112k» y «112.000» → 112000).
fn place_words(s: &str) -> std::collections::HashSet<String> {
    let text = fold(&crate::itn::normalize_numbers(&s.replace('.', "")));
    text.split(' ')
        .map(|w| match w.strip_suffix('k') {
            Some(n) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => format!("{n}000"),
            _ => w.to_owned(),
        })
        .flat_map(|w| words(&w))
        .collect()
}

/// Palabras de un apartado de la plantilla (título y descripción), sin tildes.
fn section_words(s: &TemplateSection) -> std::collections::HashSet<String> {
    words(&fold(&format!("{} {}", s.heading, s.hint)))
}

/// Salida del modelo encajada en una plantilla.
#[derive(Debug, Clone)]
struct Fit {
    /// Un apartado por cada uno de la plantilla, en su orden, más «Otros
    /// temas» al final (vacío si todo encajó).
    sections: Vec<RawSection>,
    /// Secciones del modelo que encajaron por título.
    matched_by_title: usize,
    /// Viñetas que acabaron en «Otros temas».
    unmatched_bullets: usize,
    bullets: usize,
    /// Títulos que escribió el modelo (para el mensaje de reintento).
    model_headings: Vec<String>,
}

pub(crate) const OTHER_TOPICS: &str = "Otros temas";

/// Encaja las secciones del modelo en los apartados de la plantilla:
/// 1. cada sección va al apartado cuyo título encaja ([`best_heading`]) o, si
///    es de tareas, al apartado `Tasks`;
/// 2. si no encaja, cada viñeta va al apartado `Topics` con más palabras en
///    común con su título y su descripción, y si no hay ninguno, a «Otros temas».
///
/// Nada de lo que escribió el modelo se pierde, y las viñetas repetidas dentro
/// de un apartado se quitan.
fn fit_to_template(raw: Vec<RawSection>, template: &[TemplateSection]) -> Fit {
    let headings: Vec<&str> = template.iter().map(|s| s.heading.as_str()).collect();
    let tasks = template.iter().position(|s| s.kind == SectionKind::Tasks);
    let topic_words: Vec<(usize, std::collections::HashSet<String>)> = template
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind == SectionKind::Topics)
        .map(|(i, s)| (i, section_words(s)))
        .collect();
    let mut out: Vec<RawSection> = template.iter().map(|s| RawSection { heading: s.heading.clone(), bullets: Vec::new() }).collect();
    out.push(RawSection { heading: OTHER_TOPICS.into(), bullets: Vec::new() });
    let mut fit = Fit { sections: Vec::new(), matched_by_title: 0, unmatched_bullets: 0, bullets: 0, model_headings: Vec::new() };
    fn push(target: &mut RawSection, b: RawBullet) {
        if !target.bullets.iter().any(|x| x.text.to_lowercase() == b.text.to_lowercase()) {
            target.bullets.push(b);
        }
    }
    for s in raw {
        fit.model_headings.push(s.heading.clone());
        fit.bullets += s.bullets.len();
        // Lo que el propio modelo puso en «Otros temas» se queda ahí (salvo que
        // la plantilla tenga un apartado con ese nombre).
        if crate::language::is_other_topics(&s.heading) && best_heading(&s.heading, &headings).is_none() {
            for b in s.bullets {
                fit.unmatched_bullets += 1;
                push(out.last_mut().unwrap(), b);
            }
            continue;
        }
        let by_title = best_heading(&s.heading, &headings);
        if by_title.is_some() {
            fit.matched_by_title += 1;
        }
        match by_title.or(tasks.filter(|_| is_next_steps(&s.heading))) {
            Some(i) => s.bullets.into_iter().for_each(|b| push(&mut out[i], b)),
            None => {
                for b in s.bullets {
                    let bw = place_words(&format!("{} {}", s.heading, b.text));
                    let best = topic_words
                        .iter()
                        .map(|(i, tw)| (*i, bw.intersection(tw).count()))
                        .filter(|(_, n)| *n > 0)
                        .max_by_key(|(i, n)| (*n, std::cmp::Reverse(*i)));
                    match best {
                        Some((i, _)) => push(&mut out[i], b),
                        None => {
                            fit.unmatched_bullets += 1;
                            push(out.last_mut().unwrap(), b);
                        }
                    }
                }
            }
        }
    }
    drop_cross_duplicates(&mut out, template);
    fit.sections = out;
    fit
}

/// Quita las viñetas que repiten, en otro apartado, lo que ya dice otra: citan
/// alguna línea en común y comparten al menos la mitad de sus términos. Se
/// queda la del apartado de tareas si una de las dos está ahí; si no, la del
/// apartado que mejor encaja con la viñeta (por título y descripción) y, a
/// igualdad, la del primero. Así un apartado del que no se
/// habló no se rellena repitiendo otro (medido: «Feedback» en el 1:1).
fn drop_cross_duplicates(sections: &mut [RawSection], template: &[TemplateSection]) {
    let fitness = |i: usize, terms: &std::collections::HashSet<String>| {
        template.get(i).map_or(0, |t| section_words(t).intersection(terms).count())
    };
    let mut items: Vec<(usize, usize, std::collections::HashSet<String>)> = Vec::new();
    for (i, s) in sections.iter().enumerate() {
        for (j, b) in s.bullets.iter().enumerate() {
            items.push((i, j, key_terms(&b.text)));
        }
    }
    let mut drop: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    for a in 0..items.len() {
        for b in a + 1..items.len() {
            let ((ia, ja, ta), (ib, jb, tb)) = (&items[a], &items[b]);
            if ia == ib || drop.contains(&(*ia, *ja)) || drop.contains(&(*ib, *jb)) {
                continue;
            }
            let (ra, rb) = (&sections[*ia].bullets[*ja].refs, &sections[*ib].bullets[*jb].refs);
            let small = ta.len().min(tb.len());
            if small == 0 || !ra.iter().any(|r| rb.contains(r)) || ta.intersection(tb).count() * 2 < small {
                continue;
            }
            // Una tarea repetida se queda en el apartado de tareas: si no, se
            // pierde como tarea (medido en el stand-up).
            let is_tasks = |i: usize| template.get(i).is_some_and(|t| t.kind == SectionKind::Tasks);
            let both: std::collections::HashSet<String> = ta.union(tb).cloned().collect();
            let loser = match (is_tasks(*ia), is_tasks(*ib)) {
                (true, false) => (*ib, *jb),
                (false, true) => (*ia, *ja),
                _ if fitness(*ib, &both) > fitness(*ia, &both) => (*ia, *ja),
                _ => (*ib, *jb),
            };
            drop.insert(loser);
        }
    }
    for (i, s) in sections.iter_mut().enumerate() {
        let mut j = 0;
        s.bullets.retain(|_| {
            let keep = !drop.contains(&(i, j));
            j += 1;
            keep
        });
    }
}

/// `` `## A`, `## B` y `## C` ``
fn heading_list(headings: &[&str], last_sep: &str) -> String {
    let items: Vec<String> = headings.iter().map(|h| format!("`## {h}`")).collect();
    match items.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{}{last_sep}{last}", rest.join(", ")),
    }
}

/// Motivo por el que una salida no sirve para la plantilla (y que se le dice
/// al modelo en el reintento), o `None` si sirve.
fn template_failure(fit: &Fit, report: &EnhanceReport, template: &[TemplateSection]) -> Option<String> {
    let expected: Vec<&str> = template.iter().map(|s| s.heading.as_str()).collect();
    let expected = heading_list(&expected, ", ");
    if fit.matched_by_title == 0 || fit.unmatched_bullets * 2 > fit.bullets {
        let wrote: Vec<&str> = fit.model_headings.iter().map(String::as_str).collect();
        let what = if wrote.is_empty() {
            "no escribiste ningún apartado con viñetas".to_owned()
        } else {
            format!("escribiste {}", heading_list(&wrote, " y "))
        };
        return Some(format!(
            "El intento anterior no siguió la plantilla: {what}. Usa exactamente estos títulos y en este orden: {expected}."
        ));
    }
    if report.empty_sections * 2 > template.len() {
        let empty: Vec<&str> = report.empty_headings.iter().map(String::as_str).collect();
        return Some(format!(
            "El intento anterior dejó sin viñetas {}. Si en la transcripción se habló de esos apartados, escribe sus viñetas con sus números de línea; \
             si no se habló, déjalos vacíos. Usa exactamente estos títulos y en este orden: {expected}.",
            heading_list(&empty, " y ")
        ));
    }
    None
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
/// cada uno va a la sección (que no sea de tareas: `tasks[i]`) cuyas citas
/// están más cerca de las suyas. Se descartan los que no citan nada válido y
/// los que se solapan con una viñeta existente (citas cercanas y palabras en
/// común).
fn add_details(sections: &mut [RawSection], details: Vec<RawBullet>, n_lines: usize, tasks: &[bool]) {
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
            .filter(|(i, _)| !tasks[*i])
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
    /// Con plantilla: no se pudo aplicar y las notas se generaron en Auto.
    pub template_fallback: bool,
    /// Con plantilla: intentos repetidos por no encajar (0 o 1).
    pub template_retries: usize,
    /// Con plantilla: viñetas del modelo que fueron a «Otros temas».
    pub unmatched_bullets: usize,
    /// Con plantilla: apartados que quedaron con «No se habló de esto».
    pub empty_sections: usize,
    pub empty_headings: Vec<String>,
}

/// Cómo se interpretan las secciones que recibe [`resolve`].
#[derive(Clone, Copy)]
enum Layout<'a> {
    /// Temas libres: las secciones sin viñetas citadas desaparecen y las
    /// notas del usuario sin sitio van a «Tus notas».
    Auto,
    /// Salida de [`fit_to_template`]: un apartado por cada uno de la
    /// plantilla más «Otros temas» al final.
    Template { sections: &'a [TemplateSection], language: &'a str },
}

/// Viñeta de una nota del usuario.
fn user_bullet(b: &UserBlock, note_id: Id) -> Bullet {
    let (is_action_item, done) = match b.kind {
        BlockKind::Task { checked } => (true, checked),
        _ => (false, false),
    };
    Bullet {
        text: b.text.clone(),
        is_user_text: true,
        source: Some(Citation::UserNote { note_id, block_id: b.id.clone() }),
        is_action_item,
        done,
    }
}

fn resolve(
    raw: Vec<RawSection>,
    segments: &[Segment],
    blocks: &[UserBlock],
    note_id: Id,
    layout: Layout<'_>,
) -> Result<(EnhancedNotes, EnhanceReport), EnhanceError> {
    let mut report = EnhanceReport::default();
    let template = match layout {
        Layout::Template { sections, .. } => Some(sections),
        Layout::Auto => None,
    };
    let is_tasks = |i: usize, heading: &str| match template {
        Some(t) => t.get(i).is_some_and(|s| s.kind == SectionKind::Tasks),
        None => is_next_steps(heading),
    };
    let mut sections: Vec<Section> = raw
        .into_iter()
        .enumerate()
        .map(|(i, rs)| {
            let action = is_tasks(i, &rs.heading);
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
                        done: false,
                    })
                })
                .collect();
            Section { heading: rs.heading, bullets }
        })
        .collect();
    if template.is_none() {
        sections.retain(|s| !s.bullets.is_empty());
        if sections.is_empty() && blocks.is_empty() {
            return Err(EnhanceError::NoSections("ninguna viñeta cita la transcripción".into()));
        }
    }

    // Notas del usuario, por orden de prioridad:
    // 1. una tarea va al apartado de tareas (en Auto, «Próximos pasos», que se
    //    crea si no existe; con una plantilla sin tareas, como nota normal);
    // 2. una nota bajo un título del usuario que encaja con un apartado, a ese apartado;
    // 3. al apartado (que no sea de tareas) con más palabras en común;
    // 4. si no, en Auto a «Tus notas» al principio y con plantilla a «Otros temas».
    // Siempre al principio del apartado y en el orden en que las escribió.
    let mut tasks_idx = (0..sections.len()).find(|&i| is_tasks(i, &sections[i].heading));
    let mut placed: Vec<Vec<Bullet>> = vec![Vec::new(); sections.len()];
    let mut orphans = Vec::new();
    let mut user_heading: Option<&str> = None;
    for (n, b) in blocks.iter().enumerate() {
        if let BlockKind::Heading(_) = b.kind {
            user_heading = Some(&b.text);
            // Un título con notas debajo solo sirve para colocarlas; uno suelto
            // se coloca como cualquier nota.
            if blocks.get(n + 1).is_some_and(|next| !matches!(next.kind, BlockKind::Heading(_))) {
                continue;
            }
        }
        let bullet = user_bullet(b, note_id);
        if matches!(b.kind, BlockKind::Task { .. }) && (template.is_none() || tasks_idx.is_some()) {
            let i = *tasks_idx.get_or_insert_with(|| {
                sections.push(Section { heading: "Próximos pasos".into(), bullets: Vec::new() });
                placed.push(Vec::new());
                sections.len() - 1
            });
            placed[i].push(bullet);
            continue;
        }
        let bullet = Bullet { is_action_item: false, done: false, ..bullet };
        let candidates: Vec<usize> = (0..sections.len()).filter(|&i| Some(i) != tasks_idx).collect();
        let by_heading = user_heading.filter(|h| *h != b.text).and_then(|h| {
            let heads: Vec<&str> = candidates.iter().map(|&i| sections[i].heading.as_str()).collect();
            best_heading(h, &heads).map(|k| candidates[k])
        });
        let bw = place_words(&b.text);
        let by_words = || {
            candidates
                .iter()
                .map(|&i| {
                    let s = &sections[i];
                    let mut sw: std::collections::HashSet<String> = std::iter::once(s.heading.as_str())
                        .chain(s.bullets.iter().map(|b| b.text.as_str()))
                        .flat_map(place_words)
                        .collect();
                    if let Some(ts) = template.and_then(|t| t.get(i)) {
                        sw.extend(section_words(ts));
                    }
                    (i, bw.intersection(&sw).count())
                })
                .filter(|(_, n)| *n > 0)
                .max_by_key(|(i, n)| (*n, std::cmp::Reverse(*i)))
                .map(|(i, _)| i)
        };
        match by_heading.or_else(by_words) {
            Some(i) => placed[i].push(bullet),
            None if template.is_some() => placed[sections.len() - 1].push(bullet),
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

    match layout {
        Layout::Template { sections: tpl, language } => {
            // «Otros temas» solo si tiene algo; los apartados vacíos de la
            // plantilla se ven, con una línea que pone el código.
            if sections.last().is_some_and(|s| s.bullets.is_empty()) {
                sections.pop();
            } else if let Some(other) = sections.get_mut(tpl.len()) {
                other.heading = crate::language::localize_heading(OTHER_TOPICS, language);
            }
            for s in sections.iter_mut().take(tpl.len()).filter(|s| s.bullets.is_empty()) {
                report.empty_sections += 1;
                report.empty_headings.push(s.heading.clone());
                s.bullets.push(Bullet {
                    text: crate::language::not_discussed(language).into(),
                    is_user_text: false,
                    source: None,
                    is_action_item: false,
                    done: false,
                });
            }
        }
        Layout::Auto => {}
    }

    report.sections = sections.len();
    report.bullets = sections.iter().map(|s| s.bullets.len()).sum();
    report.action_items = sections.iter().flat_map(|s| &s.bullets).filter(|b| b.is_action_item).count();
    let notes = EnhancedNotes { sections, edited: false };
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

/// Ejecuta el enhance con opciones explícitas. Con plantilla
/// (`input.template`), encaja la salida en ella; si no se puede, reintenta una
/// vez y después genera en Auto con `template_fallback`.
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
        return resolve(Vec::new(), input.segments, &blocks, input.note.id, Layout::Auto);
    }
    let gen = GenOptions { max_tokens: Some(summary_budget(words, opts.max_tokens)), seed: opts.seed, ..Default::default() };
    let Some(template) = input.template else {
        return enhance_auto(model, input, &blocks, opts, &gen, None).await;
    };
    if let Err(e) = template.validate() {
        trace(&format!("plantilla «{}» no válida ({e}): se usa Auto", template.name));
        let (notes, mut report) = enhance_auto(model, input, &blocks, opts, &gen, None).await?;
        report.template_fallback = true;
        return Ok((notes, report));
    }
    let tpl = &template.structure;
    let layout = Layout::Template { sections: tpl, language: input.summary_language };
    // Con plantilla el modelo escribe más (todos los apartados, a veces con su
    // descripción en el título): con el tope de Auto se cortaba antes de llegar
    // a las tareas, que van al final (medido en el 1:1).
    let auto_gen = gen.clone();
    let gen = GenOptions { max_tokens: Some(template_budget(words, tpl.len(), opts.max_tokens)), ..gen };
    let mut reason: Option<String> = None;
    let mut chosen = None;
    // Las pasadas de detalle salen en el lote del primer intento y sirven
    // también para el reintento y para la caída a Auto.
    let mut details: Option<Vec<String>> = None;
    for attempt in 0..2 {
        // El reintento cambia de semilla (si se fijó) y lleva el motivo del fallo.
        let grammar = opts.markdown_grammar.then(|| markdown_grammar(tpl, input.summary_language));
        let gen = GenOptions { seed: gen.seed.map(|s| s.wrapping_add(attempt)), grammar, ..gen.clone() };
        let main = (template_messages(input, template, &blocks, opts, reason.as_deref()), gen);
        let md = complete_with_details(model, main, &mut details, detail_requests(input, opts)).await?;
        trace(&format!("resumen con plantilla «{}», intento {}:\n{md}", template.name, attempt + 1));
        let fit = fit_to_template(parse_markdown(&md), tpl);
        let (_, report) = resolve(fit.sections.clone(), input.segments, &blocks, input.note.id, layout)?;
        match template_failure(&fit, &report, tpl) {
            None => {
                chosen = Some((fit, attempt as usize));
                break;
            }
            Some(r) => {
                trace(&format!("no encaja: {r}"));
                reason = Some(r);
            }
        }
    }
    let Some((fit, retries)) = chosen else {
        let (notes, mut report) = enhance_auto(model, input, &blocks, opts, &auto_gen, details).await?;
        report.template_fallback = true;
        report.template_retries = 1;
        return Ok((notes, report));
    };
    let mut raw = fit.sections;
    let mut tasks: Vec<bool> = tpl.iter().map(|s| s.kind == SectionKind::Tasks).collect();
    tasks.push(false);
    apply_details(&mut raw, details.as_deref().unwrap_or_default(), input.segments.len(), &tasks);
    let (notes, mut report) = resolve(raw, input.segments, &blocks, input.note.id, layout)?;
    report.template_retries = retries;
    report.unmatched_bullets = fit.unmatched_bullets;
    Ok((notes, report))
}

fn trace(msg: &str) {
    if std::env::var_os("JUANOLA_LLM_TRACE").is_some() {
        eprintln!("[enhance] {msg}\n[/enhance]");
    }
}

/// Pasadas de detalle por fragmentos: datos concretos que el resumen omitió.
/// No dependen del resumen (solo se juntan al final), así que se generan en el
/// mismo lote que él. (Resumir por fragmentos y reagrupar los temas con el
/// modelo se probó y daba peor estructura y perdía tareas.)
fn detail_requests(input: &EnhanceInput<'_>, opts: &EnhanceOptions) -> Vec<(Vec<Message>, GenOptions)> {
    let Some(secs) = opts.detail_chunk_secs else {
        return Vec::new();
    };
    chunk_ranges(input.segments, secs)
        .into_iter()
        .map(|range| {
            let segs = &input.segments[range.clone()];
            let prompt = juanola_prompts::ENHANCE_DETAILS
                .replace("{{from}}", &clock(segs[0].start))
                .replace("{{to}}", &clock(segs[segs.len() - 1].end))
                .replace("{{transcript}}", format_transcript_from(segs, range.start + 1, opts.itn).trim_end())
                .replace("{{language}}", input.summary_language)
                + &crate::language::prompt_suffix(input.summary_language);
            // 12 datos caben en ~450 tokens; el tope corta las listas que el
            // modelo alarga sin fin (es lo que más tiempo cuesta).
            let gen = GenOptions { max_tokens: Some(450), seed: opts.seed, ..Default::default() };
            (vec![Message { role: "user".into(), content: prompt }], gen)
        })
        .collect()
}

/// Añade a `raw` los datos de las pasadas de detalle (nada si el resumen no
/// dio ninguna viñeta).
fn apply_details(raw: &mut [RawSection], details_md: &[String], n_lines: usize, tasks: &[bool]) {
    if raw.iter().all(|s| s.bullets.is_empty()) {
        return;
    }
    for md in details_md {
        let details: Vec<RawBullet> = parse_markdown(md).into_iter().flat_map(|s| s.bullets).collect();
        add_details(raw, details, n_lines, tasks);
    }
}

/// Genera `main` junto con las peticiones de detalle (si no se tienen ya) en
/// un solo lote. Devuelve la salida de `main` y la de los detalles.
async fn complete_with_details(
    model: &dyn ChatModel,
    main: (Vec<Message>, GenOptions),
    details: &mut Option<Vec<String>>,
    detail_reqs: Vec<(Vec<Message>, GenOptions)>,
) -> Result<String, EnhanceError> {
    if details.is_some() {
        return Ok(model.complete(&main.0, &main.1).await?);
    }
    let mut reqs = vec![main];
    reqs.extend(detail_reqs);
    let mut out = model.complete_many(&reqs).await?.into_iter();
    let md = out.next().unwrap_or_default();
    *details = Some(out.collect());
    Ok(md)
}

/// Enhance sin plantilla: temas libres. `details` trae las pasadas de detalle
/// si ya se generaron (caída desde una plantilla).
async fn enhance_auto(
    model: &dyn ChatModel,
    input: &EnhanceInput<'_>,
    blocks: &[UserBlock],
    opts: &EnhanceOptions,
    gen: &GenOptions,
    mut details: Option<Vec<String>>,
) -> Result<(EnhancedNotes, EnhanceReport), EnhanceError> {
    // 1. Resumen en una pasada (da la estructura y las tareas) y 2. pasadas
    // de detalle por fragmentos, en el mismo lote.
    let main = (auto_messages(input, blocks, opts), gen.clone());
    let md = complete_with_details(model, main, &mut details, detail_requests(input, opts)).await?;
    trace(&format!("resumen:\n{md}"));
    let mut raw = merge_sections(parse_markdown(&md));
    let tasks: Vec<bool> = raw.iter().map(|s| is_next_steps(&s.heading)).collect();
    apply_details(&mut raw, details.as_deref().unwrap_or_default(), input.segments.len(), &tasks);
    if raw.is_empty() {
        return Err(EnhanceError::NoSections(md));
    }
    // (Se probó una pasada aparte para las tareas en JSON con gramática: daba
    // peores tareas, con frases copiadas y sin responsable.)
    let (mut notes, report) = resolve(raw, input.segments, blocks, input.note.id, Layout::Auto)?;
    for s in notes.sections.iter_mut() {
        s.heading = crate::language::localize_heading(&s.heading, input.summary_language);
    }
    Ok((notes, report))
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

    fn p(text: &str) -> serde_json::Value {
        serde_json::json!({"type":"paragraph","content":[{"type":"text","text":text}]})
    }
    fn li(ty: &str, content: Vec<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({"type":ty,"content":content})
    }
    fn texts(b: &[UserBlock]) -> Vec<(&str, &str)> {
        b.iter().map(|b| (b.id.as_str(), b.text.as_str())).collect()
    }

    #[test]
    fn blocks_from_prosemirror() {
        let doc = serde_json::json!({"type":"doc","content":[
            p("presupuesto Q3"),
            {"type":"paragraph"},
            {"type":"bulletList","content":[{"type":"listItem","attrs":{"id":"x9"},"content":[p("demo jueves")]}]}
        ]});
        let b = user_blocks(&doc);
        assert_eq!(texts(&b), [("b0", "presupuesto Q3"), ("x9", "demo jueves")]);
        assert_eq!((b[0].kind, b[1].kind), (BlockKind::Paragraph, BlockKind::ListItem));
    }

    #[test]
    fn each_list_item_is_its_own_block() {
        // Antes: «demo juevesllamar a Luis» en un único bloque.
        let doc = serde_json::json!({"type":"doc","content":[
            li("bulletList", vec![li("listItem", vec![p("demo jueves")]), li("listItem", vec![p("llamar a Luis")])])
        ]});
        assert_eq!(texts(&user_blocks(&doc)), [("b0.0", "demo jueves"), ("b0.1", "llamar a Luis")]);
    }

    #[test]
    fn nested_lists_follow_their_parent() {
        let doc = serde_json::json!({"type":"doc","content":[
            li("orderedList", vec![
                li("listItem", vec![p("Precios"), li("bulletList", vec![li("listItem", vec![p("básico 39")]), li("listItem", vec![p("pro 99")])])]),
                li("listItem", vec![p("Contratación")]),
            ])
        ]});
        assert_eq!(
            texts(&user_blocks(&doc)),
            [("b0.0", "Precios"), ("b0.0.0", "básico 39"), ("b0.0.1", "pro 99"), ("b0.1", "Contratación")]
        );
    }

    #[test]
    fn task_items_keep_their_state() {
        let doc = serde_json::json!({"type":"doc","content":[{"type":"taskList","content":[
            {"type":"taskItem","attrs":{"checked":false},"content":[p("llamar a Luis")]},
            {"type":"taskItem","attrs":{"checked":true},"content":[p("enviar acta")]}
        ]}]});
        let b = user_blocks(&doc);
        assert_eq!(texts(&b), [("b0.0", "llamar a Luis"), ("b0.1", "enviar acta")]);
        assert_eq!((b[0].kind, b[1].kind), (BlockKind::Task { checked: false }, BlockKind::Task { checked: true }));
    }

    #[test]
    fn headings_emphasis_and_inline_text() {
        let doc = serde_json::json!({"type":"doc","content":[
            {"type":"heading","attrs":{"level":3},"content":[{"type":"text","text":"Contratación"}]},
            {"type":"paragraph","content":[
                {"type":"text","text":"Elena empieza "},
                {"type":"text","marks":[{"type":"bold"}],"text":"en enero"},
                {"type":"hardBreak"},
                {"type":"text","text":"con  contrato"}
            ]},
            {"type":"paragraph","content":[{"type":"text","marks":[{"type":"italic"}],"text":"sin énfasis"}]},
            {"type":"image","attrs":{"src":"data:,"}},
            {"type":"blockquote","content":[p("cita")]}
        ]});
        let b = user_blocks(&doc);
        assert_eq!(texts(&b), [("b0", "Contratación"), ("b1", "Elena empieza en enero con contrato"), ("b2", "sin énfasis"), ("b4.0", "cita")]);
        assert_eq!(b[0].kind, BlockKind::Heading(3));
        assert_eq!(b.iter().map(|b| b.emphasis).collect::<Vec<_>>(), [false, true, false, false]);
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
        add_details(&mut secs, details, 100, &[false, false, true]);
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
            UserBlock { id: "b0".into(), text: "precios nuevos: enero".into(), kind: BlockKind::Paragraph, emphasis: false },
            UserBlock { id: "b1".into(), text: "llamar a la gestoría".into(), kind: BlockKind::Paragraph, emphasis: false },
        ];
        let raw = parse_markdown("## Errores\n- Citas duplicadas [3, 2, 99]\n## Precios\n- Los precios nuevos entran en enero [1]\n- Sin cita\n## Inventado\n- Nada que lo respalde\n## Próximos pasos\n- Luis llama hoy [2]\n");
        let (notes, rep) = resolve(raw, &segs, &blocks, Id::nil(), Layout::Auto).unwrap();
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

    // --- Plantillas -------------------------------------------------------

    fn tpl_sections(spec: &[(&str, &str, SectionKind)]) -> Vec<TemplateSection> {
        spec.iter().map(|(h, hint, k)| TemplateSection { heading: (*h).into(), hint: (*hint).into(), kind: *k }).collect()
    }

    fn weekly() -> Vec<TemplateSection> {
        tpl_sections(&[
            ("Métricas y estado", "cifras, resultados y cómo van los proyectos", SectionKind::Topics),
            ("Temas tratados", "asuntos de los que se habló", SectionKind::Topics),
            ("Decisiones", "lo que se acordó o decidió", SectionKind::Topics),
            ("Próximos pasos", "cada tarea: quién, qué y cuándo", SectionKind::Tasks),
        ])
    }

    fn heads(f: &Fit) -> Vec<&str> {
        f.sections.iter().map(|s| s.heading.as_str()).collect()
    }

    fn texts_of(s: &RawSection) -> Vec<&str> {
        s.bullets.iter().map(|b| b.text.as_str()).collect()
    }

    #[test]
    fn heading_matching_levels() {
        assert_eq!(heading_match("Metricas y Estado:", "Métricas y estado"), Some(3));
        assert_eq!(heading_match("Decisiones tomadas", "Decisiones"), Some(2));
        // El modelo copia la descripción detrás del título.
        assert_eq!(heading_match("Métricas y estado — cifras y resultados", "Métricas y estado"), Some(2));
        assert_eq!(heading_match("Estado de las métricas", "Métricas y estado"), Some(1));
        assert_eq!(heading_match("Contratación", "Decisiones"), None);
        // «Temas» no está dentro de «Tematica» como palabra entera.
        assert_eq!(heading_match("Temática", "Temas"), None);
    }

    #[test]
    fn fit_keeps_template_order_and_places_everything() {
        let md = "## Decisiones tomadas\n- Se aprueba el plan [5]\n\
                  ## metricas y estado\n- MRR de 112000 euros [1]\n- MRR de 112000 euros [1]\n\
                  ## Contratación\n- Elena empieza en enero [7]\n- Se decidió subir precios [8]\n\
                  ## Tareas pendientes\n- Luis llama el lunes [9]\n";
        let f = fit_to_template(parse_markdown(md), &weekly());
        assert_eq!(heads(&f), ["Métricas y estado", "Temas tratados", "Decisiones", "Próximos pasos", OTHER_TOPICS]);
        assert_eq!(texts_of(&f.sections[0]), ["MRR de 112000 euros"], "repetidas fuera");
        assert_eq!(texts_of(&f.sections[2]), ["Se aprueba el plan", "Se decidió subir precios"], "por palabras con «decidió»/«acordó o decidió»");
        assert_eq!(texts_of(&f.sections[3]), ["Luis llama el lunes"], "una sección de tareas va al apartado Tasks");
        assert_eq!(texts_of(&f.sections[4]), ["Elena empieza en enero"], "sin palabras en común: «Otros temas»");
        assert_eq!((f.matched_by_title, f.unmatched_bullets, f.bullets), (2, 1, 6));
        assert_eq!(f.model_headings, ["Decisiones tomadas", "metricas y estado", "Contratación", "Tareas pendientes"]);
    }

    #[test]
    fn retry_message_names_what_was_written_and_expected() {
        let f = fit_to_template(parse_markdown("## Resumen\n- a [1]\n## Varios\n- b [2]\n"), &weekly());
        let rep = EnhanceReport::default();
        let msg = template_failure(&f, &rep, &weekly()).expect("no encaja por título");
        assert!(msg.contains("escribiste `## Resumen` y `## Varios`"), "{msg}");
        assert!(msg.contains("`## Métricas y estado`, `## Temas tratados`, `## Decisiones`, `## Próximos pasos`"), "{msg}");
        // Encaja por título pero deja vacía más de la mitad.
        let f = fit_to_template(parse_markdown("## Decisiones\n- a [1]\n"), &weekly());
        let rep = EnhanceReport { empty_sections: 3, empty_headings: vec!["A".into(), "B".into(), "C".into()], ..Default::default() };
        let msg = template_failure(&f, &rep, &weekly()).unwrap();
        assert!(msg.contains("sin viñetas `## A`, `## B` y `## C`"), "{msg}");
        let rep = EnhanceReport { empty_sections: 2, ..Default::default() };
        assert_eq!(template_failure(&f, &rep, &weekly()), None, "la mitad justa todavía sirve");
    }

    fn three_segs() -> Vec<Segment> {
        (0..10).map(|i| seg(i, Channel::System, i as f64 * 5.0, i as f64 * 5.0 + 4.0, "x")).collect()
    }

    fn ub(id: &str, text: &str, kind: BlockKind) -> UserBlock {
        UserBlock { id: id.into(), text: text.into(), kind, emphasis: false }
    }

    #[test]
    fn template_sections_are_all_shown_and_empty_ones_marked() {
        let segs = three_segs();
        let f = fit_to_template(parse_markdown("## Decisiones\n- Se aprueba el plan [2]\n- Inventado sin cita\n"), &weekly());
        let layout = Layout::Template { sections: &weekly(), language: "inglés" };
        let (notes, rep) = resolve(f.sections, &segs, &[], Id::nil(), layout).unwrap();
        let h: Vec<&str> = notes.sections.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(h, ["Métricas y estado", "Temas tratados", "Decisiones", "Próximos pasos"], "los títulos de la plantilla no se traducen; sin «Otros temas» vacío");
        let empty = &notes.sections[0].bullets;
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0].text, "Not discussed");
        assert!(empty[0].source.is_none() && !empty[0].is_user_text && !empty[0].is_action_item);
        assert_eq!((rep.empty_sections, rep.uncited), (3, 1));
        assert_eq!(rep.empty_headings, ["Métricas y estado", "Temas tratados", "Próximos pasos"]);
    }

    #[test]
    fn tasks_are_marked_by_kind_and_other_topics_is_localized() {
        let segs = three_segs();
        let tpl = tpl_sections(&[("Bloqueos", "lo que impide avanzar", SectionKind::Topics), ("Acciones", "", SectionKind::Tasks)]);
        let f = fit_to_template(parse_markdown("## Acciones\n- Ana revisa el contrato [3]\n## Viaje\n- Barcelona en marzo [4]\n"), &tpl);
        let (notes, rep) = resolve(f.sections, &segs, &[], Id::nil(), Layout::Template { sections: &tpl, language: "inglés" }).unwrap();
        assert!(notes.sections[1].bullets[0].is_action_item, "«Acciones» no es un título de tareas conocido, pero es Tasks");
        assert_eq!(notes.sections[2].heading, "Other topics");
        assert!(!notes.sections[2].bullets[0].is_action_item);
        assert_eq!(rep.empty_sections, 1);
    }

    #[test]
    fn user_notes_with_template() {
        let segs = three_segs();
        // Lo que va detrás de un título queda bajo él hasta el siguiente título.
        let blocks = [
            ub("b0", "cumpleaños de Marta", BlockKind::Paragraph),
            ub("b1", "MRR sube", BlockKind::Paragraph),
            ub("b2", "Decisiones", BlockKind::Heading(2)),
            ub("b2.0", "nada de rebajas", BlockKind::ListItem),
            ub("b3", "llamar a Luis", BlockKind::Task { checked: true }),
        ];
        let f = fit_to_template(parse_markdown("## Métricas y estado\n- El MRR sube un 4 % [1]\n"), &weekly());
        let (notes, rep) = resolve(f.sections, &segs, &blocks, Id::nil(), Layout::Template { sections: &weekly(), language: "español" }).unwrap();
        let sec = |h: &str| notes.sections.iter().find(|s| s.heading == h).unwrap_or_else(|| panic!("falta {h}"));
        let dec = sec("Decisiones");
        assert_eq!(dec.bullets.len(), 1, "solo la nota del usuario, sin «No se habló de esto»");
        assert!(dec.bullets[0].is_user_text && dec.bullets[0].text == "nada de rebajas");
        let tasks = sec("Próximos pasos");
        assert!(tasks.bullets[0].is_action_item && tasks.bullets[0].done && tasks.bullets[0].text == "llamar a Luis");
        assert_eq!(sec("Métricas y estado").bullets[0].text, "MRR sube", "por palabras, al principio");
        assert_eq!(sec("Otros temas").bullets[0].text, "cumpleaños de Marta");
        assert!(notes.sections.iter().all(|s| s.heading != "Tus notas"));
        assert!(notes.sections.iter().all(|s| s.heading != "Decisiones" || s.bullets.iter().all(|b| b.text != "Decisiones")), "el título del usuario no se coloca");
        assert_eq!((rep.user_bullets, rep.empty_sections), (4, 1));
        match &dec.bullets[0].source {
            Some(Citation::UserNote { block_id, .. }) => assert_eq!(block_id, "b2.0"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn user_tasks_in_auto_go_to_next_steps() {
        let segs = three_segs();
        let blocks = [ub("b0", "enviar la oferta", BlockKind::Task { checked: false }), ub("b1", "sin relación", BlockKind::Paragraph)];
        let raw = parse_markdown("## Precios\n- Planes nuevos [1]\n");
        let (notes, _) = resolve(raw, &segs, &blocks, Id::nil(), Layout::Auto).unwrap();
        let h: Vec<&str> = notes.sections.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(h, ["Tus notas", "Precios", "Próximos pasos"], "se crea «Próximos pasos» para la tarea");
        assert!(notes.sections[2].bullets[0].is_action_item && !notes.sections[2].bullets[0].done);
        // Una plantilla sin apartado de tareas: la tarea se coloca como nota normal.
        let tpl = tpl_sections(&[("Oferta", "", SectionKind::Topics)]);
        let f = fit_to_template(parse_markdown("## Oferta\n- Precio cerrado [2]\n"), &tpl);
        let (notes, _) = resolve(f.sections, &segs, &blocks[..1], Id::nil(), Layout::Template { sections: &tpl, language: "español" }).unwrap();
        assert_eq!(notes.sections[0].bullets[0].text, "enviar la oferta");
        assert!(!notes.sections[0].bullets[0].is_action_item);
    }

    #[test]
    fn user_notes_in_the_prompt() {
        let blocks = [
            ub("b0", "Contratación", BlockKind::Heading(2)),
            ub("b1", "Elena empieza en enero", BlockKind::ListItem),
            ub("b2", "llamar a Luis", BlockKind::Task { checked: false }),
            UserBlock { emphasis: true, ..ub("b3", "presupuesto Q3", BlockKind::Paragraph) },
            ub("b4", "Riesgos", BlockKind::Heading(3)),
            ub("b5", "retraso del proveedor", BlockKind::ListItem),
        ];
        let n = format_user_notes(&blocks);
        assert!(n.contains("marcan lo que le importó"));
        assert!(n.ends_with(
            "## Contratación\n- Elena empieza en enero\n- [tarea] llamar a Luis\n- [importante] presupuesto Q3\n## Riesgos\n- [importante] retraso del proveedor\n"
        ), "{n}");
        assert_eq!(format_user_notes(&[]), "");
    }

    #[test]
    fn template_prompt_lists_sections_and_retry_reason() {
        let n = note(serde_json::json!({"type": "doc", "content": []}));
        let segs = three_segs();
        let t = Template {
            id: Id::nil(),
            name: "Semanal".into(),
            purpose: "revisar el estado".into(),
            style: "conciso".into(),
            structure: weekly(),
            is_builtin: true,
            shared_with_workspace: false,
        };
        let input = EnhanceInput { note: &n, segments: &segs, template: Some(&t), summary_language: "español", user_profile: "" };
        let m = &template_messages(&input, &t, &[], &EnhanceOptions::default(), Some("MOTIVO"))[0].content;
        assert!(m.contains("Plantilla «Semanal». Propósito: revisar el estado. Estilo: conciso."));
        assert!(m.contains("## Métricas y estado — cifras, resultados y cómo van los proyectos\n## Temas tratados"));
        assert!(!m.contains("{{"), "{m}");
        assert!(m.trim_end().ends_with("MOTIVO"));
    }

    /// Modelo falso: responde con `template` a los prompts con plantilla y con
    /// `auto` a los demás, y cuenta las llamadas.
    struct Scripted {
        template: Vec<&'static str>,
        auto: &'static str,
        calls: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl ChatModel for Scripted {
        fn context_window(&self) -> usize {
            8192
        }
        async fn complete(&self, m: &[Message], _: &GenOptions) -> Result<String, juanola_llm::LlmError> {
            let mut calls = self.calls.lock().unwrap();
            calls.push(m[0].content.clone());
            let n_tpl = calls.iter().filter(|c| c.contains("usando exactamente estos apartados")).count();
            Ok(if m[0].content.contains("usando exactamente estos apartados") {
                self.template[(n_tpl - 1).min(self.template.len() - 1)].into()
            } else {
                self.auto.into()
            })
        }
        fn stream<'a>(&'a self, _: &'a [Message], _: &'a GenOptions) -> futures::stream::BoxStream<'a, Result<String, juanola_llm::LlmError>> {
            unreachable!()
        }
    }

    fn long_segs() -> Vec<Segment> {
        let text = "Hoy repasamos las métricas del mes y decidimos subir los precios del plan básico";
        (0..6).map(|i| seg(i, Channel::System, i as f64 * 10.0, i as f64 * 10.0 + 9.0, text)).collect()
    }

    fn weekly_template() -> Template {
        crate::templates::builtin_template(crate::templates::WEEKLY_TEAM, "español").unwrap()
    }

    const GOOD: &str = "## Métricas y estado\n- MRR de 112000 euros [1]\n## Temas tratados\n- Precios del plan básico [2]\n## Decisiones\n- Se suben los precios [3]\n## Próximos pasos\n- Luis prepara la tabla [4]\n";
    const GARBAGE: &str = "## Resumen\n- Algo pasó [1]\n## Varios\n- Otra cosa [2]\n";
    const AUTO: &str = "## Precios\n- Suben los precios [2]\n## Próximos pasos\n- Luis prepara la tabla [4]\n";

    #[tokio::test]
    async fn template_retries_once_with_the_reason() {
        let n = note(serde_json::json!({"type": "doc", "content": []}));
        let segs = long_segs();
        let t = weekly_template();
        let model = Scripted { template: vec![GARBAGE, GOOD], auto: AUTO, calls: Default::default() };
        let input = EnhanceInput { note: &n, segments: &segs, template: Some(&t), summary_language: "español", user_profile: "" };
        let opts = EnhanceOptions { detail_chunk_secs: None, ..Default::default() };
        let (notes, rep) = enhance_with(&model, &input, &opts).await.unwrap();
        assert!(!rep.template_fallback);
        assert_eq!(rep.template_retries, 1);
        let h: Vec<&str> = notes.sections.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(h, ["Métricas y estado", "Temas tratados", "Decisiones", "Próximos pasos"]);
        let calls = model.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert!(calls[1].contains("El intento anterior no siguió la plantilla: escribiste `## Resumen` y `## Varios`"));
    }

    #[tokio::test]
    async fn template_falls_back_to_auto() {
        let n = note(serde_json::json!({"type": "doc", "content": []}));
        let segs = long_segs();
        let t = weekly_template();
        let model = Scripted { template: vec![GARBAGE], auto: AUTO, calls: Default::default() };
        let input = EnhanceInput { note: &n, segments: &segs, template: Some(&t), summary_language: "español", user_profile: "" };
        let opts = EnhanceOptions { detail_chunk_secs: None, ..Default::default() };
        let (notes, rep) = enhance_with(&model, &input, &opts).await.unwrap();
        assert!(rep.template_fallback);
        assert_eq!(rep.template_retries, 1);
        assert_eq!(notes.sections[0].heading, "Precios", "salida de Auto");
        assert_eq!(model.calls.lock().unwrap().len(), 3, "dos intentos con plantilla y uno en Auto");
        // Una plantilla no válida va directa a Auto.
        let mut bad = weekly_template();
        bad.structure.clear();
        let model = Scripted { template: vec![GOOD], auto: AUTO, calls: Default::default() };
        let input = EnhanceInput { template: Some(&bad), ..input };
        let (_, rep) = enhance_with(&model, &input, &opts).await.unwrap();
        assert!(rep.template_fallback && model.calls.lock().unwrap().len() == 1);
    }

    #[tokio::test]
    async fn details_are_generated_once_with_the_first_attempt() {
        let n = note(serde_json::json!({"type": "doc", "content": []}));
        let segs = long_segs();
        let t = weekly_template();
        let model = Scripted { template: vec![GARBAGE], auto: AUTO, calls: Default::default() };
        let input = EnhanceInput { note: &n, segments: &segs, template: Some(&t), summary_language: "español", user_profile: "" };
        // 60 s de reunión en fragmentos de 30 s: dos pasadas de detalle.
        let opts = EnhanceOptions { detail_chunk_secs: Some(30.0), ..Default::default() };
        let (_, rep) = enhance_with(&model, &input, &opts).await.unwrap();
        assert!(rep.template_fallback);
        let calls = model.calls.lock().unwrap();
        let kinds: Vec<&str> = calls
            .iter()
            .map(|c| if c.starts_with("Fragmento de la transcripción") { "detalle" } else if c.contains("usando exactamente estos apartados") { "plantilla" } else { "auto" })
            .collect();
        // En el lote del primer intento van los detalles; el reintento y Auto los reutilizan.
        assert_eq!(kinds, ["plantilla", "detalle", "detalle", "plantilla", "auto"]);
    }

    #[test]
    fn placement_words_normalize_numbers() {
        let w = place_words("MRR 112k, objetivo de 120.000 euros");
        assert!(w.contains("112000") && w.contains("120000") && w.contains("objetivo"), "{w:?}");
    }

    #[test]
    fn model_other_topics_stays_there_and_counts() {
        let f = fit_to_template(parse_markdown("## Decisiones\n- Se aprueba [1]\n## Other topics\n- Viaje a Lisboa [2]\n- Decisión de algo [3]\n"), &weekly());
        assert_eq!(texts_of(&f.sections[4]), ["Viaje a Lisboa", "Decisión de algo"], "no se redistribuye");
        assert_eq!((f.unmatched_bullets, f.bullets, f.matched_by_title), (2, 3, 1));
        let t = Template { id: Id::nil(), name: "x".into(), purpose: String::new(), style: String::new(), structure: weekly(), is_builtin: false, shared_with_workspace: false };
        assert!(structure_with_outlet(&t.structure, "inglés").ends_with("## Other topics — solo si hace falta: lo que se habló y no corresponde a ninguno de los apartados anteriores"));
    }

    #[test]
    fn repeated_bullets_in_another_section_are_dropped() {
        let md = "## Cómo está\n- Diego está cansado por las guardias del mes [2, 3]\n\
                  ## Feedback\n- Diego dice que está cansado y le tocan más guardias [3]\n\
                  ## Temas tratados\n- Guardias de plataforma en la rotación [9]\n";
        let tpl = tpl_sections(&[
            ("Cómo está", "estado de ánimo y carga de trabajo", SectionKind::Topics),
            ("Temas tratados", "asuntos de trabajo", SectionKind::Topics),
            ("Feedback", "comentarios sobre el trabajo", SectionKind::Topics),
        ]);
        let f = fit_to_template(parse_markdown(md), &tpl);
        assert_eq!(texts_of(&f.sections[0]).len(), 1);
        assert!(f.sections[2].bullets.is_empty(), "la repetición de «Cómo está» no rellena «Feedback»");
        assert_eq!(texts_of(&f.sections[1]).len(), 1, "sin líneas en común no es repetición");
    }

    #[test]
    fn bold_is_removed() {
        let s = parse_markdown("## A\n- Tiene **3 guardias** este mes [1]\n");
        assert_eq!(s[0].bullets[0].text, "Tiene 3 guardias este mes");
    }

    #[test]
    fn templates_get_more_tokens() {
        assert_eq!(template_budget(764, 4, 3000), 611 + 750);
        assert_eq!(template_budget(30, 1, 3000), 800);
        assert_eq!(template_budget(2400, 4, 3000), 2670);
        assert_eq!(template_budget(10_000, 4, 3000), 3000);
    }

    #[test]
    fn repeated_task_stays_in_tasks() {
        let md = "## Hecho desde la última\n- Yo renuevo el certificado de Apple esta mañana [8]\n\
                  ## Próximos pasos\n- Yo renuevo el certificado Apple esta mañana [8]\n";
        let tpl = tpl_sections(&[
            ("Hecho desde la última", "lo que cada persona ya terminó", SectionKind::Topics),
            ("Próximos pasos", "cada tarea", SectionKind::Tasks),
        ]);
        let f = fit_to_template(parse_markdown(md), &tpl);
        assert!(f.sections[0].bullets.is_empty());
        assert_eq!(f.sections[1].bullets.len(), 1);
    }

    #[test]
    fn grammar_fixes_headings_in_order() {
        let g = markdown_grammar(&weekly(), "español");
        assert!(g.starts_with("root ::= s0 s1 s2 s3 other?\n"), "{g}");
        assert!(g.contains("s0 ::= \"## Métricas y estado\" \"\\n\" body\n"), "{g}");
        assert!(g.contains("other ::= \"## Otros temas\""));
        let q = markdown_grammar(&tpl_sections(&[("Dice \"hola\"", "", SectionKind::Topics)]), "español");
        assert!(q.contains(r###""## Dice \"hola\"""###), "{q}");
    }
}
