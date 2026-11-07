//! Evalúa variantes del enhance sobre una reunión con hechos conocidos.
//!
//! ```text
//! cargo run --release -p juanola-enhance --example eval_enhance -- \
//!   <segments.json> <facts.json> [notas.txt] [--runs N] [--variant nombre=itn|noitn[+detail<seg>|+nodetail][:prompt.md]]...
//! ```
//! Métricas por ejecución:
//! - cobertura: hechos presentes / hechos alcanzables (los que el transcript
//!   contiene; si el STT se equivocó, el hecho no cuenta contra el LLM);
//! - fidelidad: números de las viñetas de la IA que aparecen en el tramo citado;
//! - estructura: secciones, temas duplicados y viñetas copiadas (8+ palabras
//!   seguidas idénticas a la transcripción).

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use juanola_core_types::{Channel, Citation, Note, NoteStatus, Segment};
use juanola_enhance::itn::normalize_numbers;
use juanola_enhance::pipelines::enhance::{enhance_with, EnhanceOptions};
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
}

#[derive(serde::Deserialize)]
struct Fact {
    id: String,
    #[serde(default)]
    task: bool,
    all: Vec<Vec<String>>,
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
}

fn score(notes: &EnhancedNotes, segs: &[Segment], facts: &[Fact], seg_norm: &[String], ngrams: &HashSet<Vec<String>>) -> Score {
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

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let segs: Vec<Seg> = serde_json::from_str(&std::fs::read_to_string(&a[0])?)?;
    let facts: Facts = serde_json::from_str(&std::fs::read_to_string(&a[1])?)?;
    let notes_path = a.get(2).filter(|p| !p.starts_with("--"));
    let mut runs = 1u32;
    let mut variants: Vec<(String, EnhanceOptions)> = Vec::new();
    let mut i = 2;
    while i < a.len() {
        match a[i].as_str() {
            "--runs" => {
                runs = a[i + 1].parse()?;
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
                variants.push((name.to_owned(), EnhanceOptions { itn, prompt, detail_chunk_secs, ..Default::default() }));
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
    let raw_doc = serde_json::json!({"type": "doc", "content": notes_txt.lines().filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::json!({"type": "paragraph", "content": [{"type": "text", "text": l}]})).collect::<Vec<_>>()});
    let now = chrono::Utc::now();
    let note = Note {
        id: note_id,
        workspace_id: uuid::Uuid::nil(),
        title: "Reunión semanal de producto".into(),
        raw_doc,
        enhanced_doc: None,
        template_id: None,
        status: NoteStatus::Enhancing,
        calendar_event_id: None,
        created_at: now,
        updated_at: now,
        trashed_at: None,
    };
    let input = EnhanceInput { note: &note, segments: &segments, template: None, summary_language: "español", user_profile: "" };
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
            let (notes, _) = enhance_with(&llm, &input, &opts).await?;
            let mut sc = score(&notes, &segments, &facts.facts, &seg_norm, &ngrams);
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
            std::fs::write(std::env::temp_dir().join(format!("juanola_eval_{name}_{r}.json")), serde_json::to_string_pretty(&notes)?).ok();
        }
    }
    Ok(())
}
