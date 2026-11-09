//! Evalúa variantes del enhance sobre una reunión con hechos conocidos.
//!
//! ```text
//! cargo run --release -p juanola-enhance --example eval_enhance -- \
//!   <segments.json> <facts.json> [notas.txt] [--runs N] [--variant nombre=itn|noitn[+detail<seg>|+nodetail][+gbnf][:prompt.md]]...
//!   [--template 1on1|standup|semanal|entrevista|ventas|plantilla.json] [--title «Título de la reunión»]
//! ```
//! `segments.json` sale del STT o, sin audio, de `tools/script_to_segments.py`.
//! Las notas admiten `# título`, `- elemento`, `- [ ] tarea`, `- [x] hecha` y
//! `**negrita**`; cada línea suelta es un párrafo.
//! Métricas por ejecución:
//! - cobertura: hechos presentes / hechos alcanzables (los que el transcript
//!   contiene; si el STT se equivocó, el hecho no cuenta contra el LLM);
//! - fidelidad: números de las viñetas de la IA que aparecen en el tramo citado;
//! - estructura: secciones, temas duplicados y viñetas copiadas (8+ palabras
//!   seguidas idénticas a la transcripción).
//!
//! Con `--template`, además (la clave de la plantilla es su nombre corto o el
//! nombre del archivo sin extensión):
//! - colocación: hechos cubiertos que están en el apartado esperado
//!   (`"section": {"semanal": ["Decisiones"]}` en el hecho);
//! - notas del usuario en su apartado (`"user_notes": {"texto": {"semanal": [...]}}`)
//!   y notas perdidas;
//! - apartados que debían quedar vacíos y se rellenaron (`"empty": {"1on1": ["Feedback"]}`);
//! - viñetas en «Otros temas», apartados vacíos, reintentos y caídas a Auto.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use juanola_core_types::{Channel, Citation, Note, NoteStatus, Segment, Template};
use juanola_enhance::itn::normalize_numbers;
use juanola_enhance::pipelines::enhance::{enhance_with, EnhanceOptions, EnhanceReport};
use juanola_enhance::templates;
use juanola_enhance::{EnhanceInput, EnhancedNotes};
use juanola_llm::backends::llamacpp::LlamaCpp;

#[derive(serde::Deserialize)]
struct Seg {
    channel: Channel,
    start: f64,
    end: f64,
    text: String,
}

#[derive(serde::Deserialize)]
struct Facts {
    facts: Vec<Fact>,
    /// Texto de la nota del usuario → plantilla → apartados aceptables.
    #[serde(default)]
    user_notes: HashMap<String, HashMap<String, Vec<String>>>,
    /// Plantilla → apartados que deben quedar vacíos en esta reunión.
    #[serde(default)]
    empty: HashMap<String, Vec<String>>,
}

#[derive(serde::Deserialize)]
struct Fact {
    id: String,
    #[serde(default)]
    task: bool,
    all: Vec<Vec<String>>,
    /// Plantilla → apartados donde puede ir el hecho.
    #[serde(default)]
    section: HashMap<String, Vec<String>>,
}

fn norm(s: &str) -> String {
    let s = normalize_numbers(s).to_lowercase();
    let s: String = s
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            c => c,
        })
        .collect();
    // Puntos de millar: 112.000 → 112000.
    let b: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for (i, c) in b.iter().enumerate() {
        let thousands = *c == '.'
            && i > 0
            && b[i - 1].is_ascii_digit()
            && b.get(i + 1..i + 4).is_some_and(|w| w.iter().all(|d| d.is_ascii_digit()))
            && !b.get(i + 4).is_some_and(|d| d.is_ascii_digit());
        if !thousands {
            out.push(*c);
        }
    }
    out
}

fn matches(fact: &Fact, text: &str) -> bool {
    fact.all.iter().all(|alts| alts.iter().any(|a| text.contains(a.as_str())))
}

fn numbers(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() || (c == ',' && !cur.is_empty()) {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(cur.trim_end_matches(',').to_owned());
            cur.clear();
        }
    }
    out
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}

#[derive(Default, Debug)]
struct Score {
    covered: usize,
    reachable: usize,
    tasks_ok: usize,
    tasks_reachable: usize,
    nums: usize,
    nums_grounded: usize,
    sections: usize,
    dup_sections: usize,
    bullets: usize,
    copied: usize,
    secs: f64,
    missed: Vec<String>,
    placed: usize,
    placeable: usize,
    misplaced: Vec<String>,
    other_bullets: usize,
    user_placed: usize,
    user_expected: usize,
    user_lost: usize,
    filled_empty: Vec<String>,
}

fn score(notes: &EnhancedNotes, segs: &[Segment], facts: &[Fact], seg_norm: &[String], ngrams: &HashSet<Vec<String>>, tkey: Option<&str>) -> Score {
    let mut sc = Score::default();
    let bullets: Vec<(String, bool)> =
        notes.sections.iter().flat_map(|s| s.bullets.iter().map(move |b| (norm(&format!("{} {}", s.heading, b.text)), b.is_action_item))).collect();
    for f in facts {
        // Alcanzable si 4 segmentos seguidos del transcript lo contienen.
        let reachable = (0..seg_norm.len()).any(|i| matches(f, &seg_norm[i..(i + 4).min(seg_norm.len())].join(" ")));
        if !reachable {
            continue;
        }
        let hit = bullets.iter().any(|(t, task)| (!f.task || *task) && matches(f, t));
        if let (true, Some(expected)) = (hit && !f.task, tkey.and_then(|k| f.section.get(k))) {
            sc.placeable += 1;
            let ok = notes.sections.iter().filter(|s| expected.iter().any(|e| e == &s.heading)).any(|s| {
                s.bullets.iter().any(|b| matches(f, &norm(&format!("{} {}", s.heading, b.text))))
            });
            sc.placed += usize::from(ok);
            if !ok {
                let landed: Vec<&str> = notes
                    .sections
                    .iter()
                    .filter(|s| s.bullets.iter().any(|b| matches(f, &norm(&format!("{} {}", s.heading, b.text)))))
                    .map(|s| s.heading.as_str())
                    .collect();
                sc.misplaced.push(format!("{} → {}", f.id, landed.join(" + ")));
            }
        }
        if f.task {
            sc.tasks_reachable += 1;
            sc.tasks_ok += usize::from(hit);
        } else {
            sc.reachable += 1;
            sc.covered += usize::from(hit);
        }
        if !hit {
            sc.missed.push(f.id.clone());
        }
    }
    for s in &notes.sections {
        for b in s.bullets.iter().filter(|b| !b.is_user_text) {
            sc.bullets += 1;
            let t = norm(&b.text);
            let w = words(&t);
            if w.windows(8).any(|g| ngrams.contains(g)) {
                sc.copied += 1;
            }
            if let Some(Citation::Transcript { start, end, .. }) = &b.source {
                let span: String = segs
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| g.end >= start - 15.0 && g.start <= end + 15.0)
                    .map(|(i, _)| seg_norm[i].as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                for n in numbers(&t) {
                    sc.nums += 1;
                    sc.nums_grounded += usize::from(span.contains(&n));
                }
            }
        }
    }
    let heads: Vec<HashSet<String>> =
        notes.sections.iter().map(|s| words(&norm(&s.heading)).into_iter().filter(|w| w.len() >= 4).collect()).collect();
    sc.sections = notes.sections.len();
    for i in 0..heads.len() {
        for j in 0..i {
            let inter = heads[i].intersection(&heads[j]).count();
            let uni = heads[i].union(&heads[j]).count();
            if uni > 0 && inter * 2 >= uni {
                sc.dup_sections += 1;
                break;
            }
        }
    }
    sc
}

/// Notas del usuario en un Markdown mínimo → documento ProseMirror (TipTap).
fn notes_doc(text: &str) -> serde_json::Value {
    fn inline(t: &str) -> Vec<serde_json::Value> {
        // `**negrita**` → marca bold; el resto, texto normal.
        let mut out = Vec::new();
        for (i, part) in t.split("**").enumerate() {
            if part.is_empty() {
                continue;
            }
            if i % 2 == 1 {
                out.push(serde_json::json!({"type": "text", "text": part, "marks": [{"type": "bold"}]}));
            } else {
                out.push(serde_json::json!({"type": "text", "text": part}));
            }
        }
        out
    }
    let mut content: Vec<serde_json::Value> = Vec::new();
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let hashes = l.chars().take_while(|c| *c == '#').count();
        if hashes > 0 {
            content.push(serde_json::json!({"type": "heading", "attrs": {"level": hashes}, "content": inline(l[hashes..].trim())}));
            continue;
        }
        let item = |ty: &str, attrs: serde_json::Value, t: &str| {
            serde_json::json!({"type": ty, "attrs": attrs, "content": [{"type": "paragraph", "content": inline(t)}]})
        };
        let (list, node) = if let Some(t) = l.strip_prefix("- [ ] ") {
            ("taskList", item("taskItem", serde_json::json!({"checked": false}), t))
        } else if let Some(t) = l.strip_prefix("- [x] ") {
            ("taskList", item("taskItem", serde_json::json!({"checked": true}), t))
        } else if let Some(t) = l.strip_prefix("- ") {
            ("bulletList", item("listItem", serde_json::json!({}), t))
        } else {
            content.push(serde_json::json!({"type": "paragraph", "content": inline(l)}));
            continue;
        };
        match content.last_mut() {
            Some(last) if last["type"] == list => last["content"].as_array_mut().unwrap().push(node),
            _ => content.push(serde_json::json!({"type": list, "content": [node]})),
        }
    }
    serde_json::json!({"type": "doc", "content": content})
}

/// Métricas que solo tienen sentido con plantilla.
fn template_score(sc: &mut Score, notes: &EnhancedNotes, facts: &Facts, tkey: Option<&str>, user_notes: usize) {
    let Some(k) = tkey else { return };
    let other = juanola_enhance::language::localize_heading("Otros temas", "español");
    sc.other_bullets = notes.sections.iter().filter(|s| s.heading == other).map(|s| s.bullets.iter().filter(|b| !b.is_user_text).count()).sum();
    let user: Vec<(&str, &str)> =
        notes.sections.iter().flat_map(|s| s.bullets.iter().filter(|b| b.is_user_text).map(move |b| (s.heading.as_str(), b.text.as_str()))).collect();
    sc.user_lost = user_notes.saturating_sub(user.len());
    for (text, by_tpl) in &facts.user_notes {
        let Some(expected) = by_tpl.get(k) else { continue };
        sc.user_expected += 1;
        sc.user_placed += usize::from(user.iter().any(|(h, t)| t == text && expected.iter().any(|e| e == h)));
    }
    let not_discussed = juanola_enhance::language::not_discussed("español");
    for h in facts.empty.get(k).into_iter().flatten() {
        let filled = notes.sections.iter().any(|s| &s.heading == h && s.bullets.iter().any(|b| b.text != not_discussed));
        if filled {
            sc.filled_empty.push(h.clone());
        }
    }
}

fn print_template(sc: &Score, r: &EnhanceReport, pct: impl Fn(usize, usize) -> f64) {
    println!(
        "               plantilla: colocación {}/{} {:.0}% · otros temas {}/{} {:.0}% · vacíos {} · notas usuario {}/{} (perdidas {}) · rellenados {:?} · reintentos {} · caída {}",
        sc.placed,
        sc.placeable,
        pct(sc.placed, sc.placeable),
        sc.other_bullets,
        sc.bullets,
        pct(sc.other_bullets, sc.bullets),
        r.empty_sections,
        sc.user_placed,
        sc.user_expected,
        sc.user_lost,
        sc.filled_empty,
        r.template_retries,
        r.template_fallback,
    );
    if !sc.misplaced.is_empty() {
        println!("               fuera de su apartado: {}", sc.misplaced.join(", "));
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let segs: Vec<Seg> = serde_json::from_str(&std::fs::read_to_string(&a[0])?)?;
    let facts: Facts = serde_json::from_str(&std::fs::read_to_string(&a[1])?)?;
    let notes_path = a.get(2).filter(|p| !p.starts_with("--"));
    let mut runs = 1u32;
    let mut template: Option<(String, Template)> = None;
    let mut title = "Reunión semanal de producto".to_owned();
    let mut variants: Vec<(String, EnhanceOptions)> = Vec::new();
    let mut i = 2;
    while i < a.len() {
        match a[i].as_str() {
            "--runs" => {
                runs = a[i + 1].parse()?;
                i += 1;
            }
            "--title" => {
                title = a[i + 1].clone();
                i += 1;
            }
            "--template" => {
                let spec = &a[i + 1];
                let id = match spec.as_str() {
                    "1on1" => Some(templates::ONE_ON_ONE),
                    "standup" => Some(templates::STAND_UP),
                    "semanal" => Some(templates::WEEKLY_TEAM),
                    "entrevista" => Some(templates::CUSTOMER_INTERVIEW),
                    "ventas" => Some(templates::SALES_CALL),
                    _ => None,
                };
                let t = match id {
                    Some(id) => templates::builtin_template(id, "español").unwrap(),
                    None => serde_json::from_str(&std::fs::read_to_string(spec)?)?,
                };
                let key = std::path::Path::new(spec).file_stem().unwrap().to_string_lossy().into_owned();
                template = Some((key, t));
                i += 1;
            }
            "--variant" => {
                let (name, spec) = a[i + 1].split_once('=').unwrap_or((&a[i + 1], "itn"));
                let (mode, prompt) = spec.split_once(':').map(|(m, p)| (m, Some(p))).unwrap_or((spec, None));
                let prompt = prompt.map(std::fs::read_to_string).transpose()?;
                let detail_chunk_secs = if mode.split('+').any(|m| m == "nodetail") {
                    None
                } else {
                    mode.split('+').find_map(|m| m.strip_prefix("detail")).and_then(|v| v.parse().ok()).or(Some(240.0))
                };
                let itn = !mode.split('+').any(|m| m == "noitn");
                let markdown_grammar = mode.split('+').any(|m| m == "gbnf");
                variants.push((name.to_owned(), EnhanceOptions { itn, prompt, detail_chunk_secs, markdown_grammar, ..Default::default() }));
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    if variants.is_empty() {
        variants.push(("por_defecto".into(), EnhanceOptions::default()));
    }

    let note_id = uuid::Uuid::new_v4();
    let segments: Vec<Segment> = segs
        .into_iter()
        .map(|s| Segment {
            id: uuid::Uuid::new_v4(),
            note_id,
            channel: s.channel,
            speaker_id: None,
            start: s.start,
            end: s.end,
            text: s.text,
            confidence: None,
            is_final: true,
        })
        .collect();
    let seg_norm: Vec<String> = segments.iter().map(|s| norm(&s.text)).collect();
    let mut ngrams = HashSet::new();
    for s in &seg_norm {
        for g in words(s).windows(8) {
            ngrams.insert(g.to_vec());
        }
    }
    let notes_txt = notes_path.map(std::fs::read_to_string).transpose()?.unwrap_or_default();
    let raw_doc = notes_doc(&notes_txt);
    let user_notes = juanola_enhance::pipelines::enhance::user_blocks(&raw_doc)
        .into_iter()
        .filter(|b| !matches!(b.kind, juanola_enhance::pipelines::enhance::BlockKind::Heading(_)))
        .count();
    let now = chrono::Utc::now();
    let note = Note {
        id: note_id,
        workspace_id: uuid::Uuid::nil(),
        title: title.clone(),
        raw_doc,
        enhanced_doc: None,
        template_id: None,
        status: NoteStatus::Enhancing,
        calendar_event_id: None,
        created_at: now,
        updated_at: now,
        trashed_at: None,
    };
    let tkey = template.as_ref().map(|(k, _)| k.as_str());
    let input =
        EnhanceInput { note: &note, segments: &segments, template: template.as_ref().map(|(_, t)| t), summary_language: "español", user_profile: "" };
    if let Some((k, t)) = &template {
        println!("plantilla: {} ({k})", t.name);
    }
    let gguf = std::env::var("JUANOLA_GGUF").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(std::env::var("HOME").unwrap())
            .join("Library/Application Support/dev.juanola.desktop/models/llm/lfm3-1.7b-q4km/LFM3-1.7B-Q4_K_M.gguf")
    });
    println!("modelo: {}", gguf.file_name().unwrap().to_string_lossy());
    let llm = LlamaCpp::load(&gguf, 32_768)?;

    println!("{:<14} {:>4} {:>9} {:>7} {:>10} {:>5} {:>4} {:>8} {:>6}", "variante", "run", "cobertura", "tareas", "nºs fieles", "secc", "dup", "copiadas", "s");
    for (name, base) in &variants {
        for r in 0..runs {
            let opts = EnhanceOptions { seed: Some(1234 + r), ..base.clone() };
            let t = Instant::now();
            let (notes, report) = enhance_with(&llm, &input, &opts).await?;
            let mut sc = score(&notes, &segments, &facts.facts, &seg_norm, &ngrams, tkey);
            template_score(&mut sc, &notes, &facts, tkey, user_notes);
            sc.secs = t.elapsed().as_secs_f64();
            let pct = |a: usize, b: usize| if b == 0 { 0.0 } else { 100.0 * a as f64 / b as f64 };
            println!(
                "{:<14} {:>4} {:>4}/{:<2}{:>3.0}% {:>3}/{:<2} {:>6.0}% ({}) {:>5} {:>4} {:>7.0}% {:>6.1}",
                name,
                r,
                sc.covered,
                sc.reachable,
                pct(sc.covered, sc.reachable),
                sc.tasks_ok,
                sc.tasks_reachable,
                pct(sc.nums_grounded, sc.nums),
                sc.nums,
                sc.sections,
                sc.dup_sections,
                pct(sc.copied, sc.bullets),
                sc.secs
            );
            println!("               faltan: {}", sc.missed.join(", "));
            if tkey.is_some() {
                print_template(&sc, &report, pct);
            }
            std::fs::write(std::env::temp_dir().join(format!("juanola_eval_{name}_{r}.json")), serde_json::to_string_pretty(&notes)?).ok();
        }
    }
    Ok(())
}
