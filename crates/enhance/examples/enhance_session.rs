//! Enhance de una reunión transcrita con LFM3 en llama.cpp.
//!
//! ```text
//! cargo run --release -p juanola-enhance --example enhance_session -- <segments.json> [notas.txt] [salida.json]
//! ```
//! `segments.json` es la salida de `juanola-stt`'s `transcribe_session`; las notas
//! del usuario son una línea por bloque. Imprime las notas con cada cita y el
//! fragmento del transcript al que apunta, para revisar que estén bien.

use std::path::PathBuf;
use std::time::Instant;

use juanola_core_types::{Channel, Citation, Note, NoteStatus, Segment};
use juanola_enhance::pipelines::enhance::{build_messages, enhance, user_blocks, EnhanceOptions};
use juanola_enhance::EnhanceInput;
use juanola_llm::backends::llamacpp::LlamaCpp;

#[derive(serde::Deserialize)]
struct Seg {
    channel: Channel,
    start: f64,
    end: f64,
    text: String,
}

fn clock(s: f64) -> String {
    format!("{:02}:{:02}", s as u64 / 60, s as u64 % 60)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let segs: Vec<Seg> = serde_json::from_str(&std::fs::read_to_string(a.first().ok_or_else(|| anyhow::anyhow!("falta segments.json"))?)?)?;
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
    let notes_txt = a.get(1).map(std::fs::read_to_string).transpose()?.unwrap_or_default();
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
    let input = EnhanceInput {
        note: &note,
        segments: &segments,
        template: None,
        summary_language: juanola_enhance::language::summary_language(&segments),
        user_profile: "",
    };

    let gguf = PathBuf::from(std::env::var("HOME")?)
        .join("Library/Application Support/dev.juanola.desktop/models/llm/lfm3-1.7b-q8/LFM3-1.7B-Q8_0.gguf");
    let llm = LlamaCpp::load(&gguf, 32_768)?;
    let prompt_tokens: usize =
        build_messages(&input, &user_blocks(&note.raw_doc), &EnhanceOptions::default()).iter().map(|m| llm.count_tokens(&m.content)).sum();
    println!("{} segmentos, {} bloques de notas, prompt ≈ {prompt_tokens} tokens", segments.len(), user_blocks(&note.raw_doc).len());

    let t = Instant::now();
    let (notes, report) = enhance(&llm, &input).await?;
    println!("enhance en {:.1}s · {report:?} · validado contra enhanced_notes_schema()\n", t.elapsed().as_secs_f64());

    for s in &notes.sections {
        println!("## {}", s.heading);
        for b in &s.bullets {
            let mark = if b.is_action_item { "☐" } else { "•" };
            match &b.source {
                Some(Citation::Transcript { start, end, .. }) => {
                    let excerpt: Vec<&str> =
                        segments.iter().filter(|g| g.start >= *start - 0.01 && g.end <= *end + 0.01).map(|g| g.text.as_str()).collect();
                    let ex = excerpt.join(" ");
                    let ex: String = ex.chars().take(140).collect();
                    println!("{mark} {}  [{}–{}]\n      ↳ «{ex}»", b.text, clock(*start), clock(*end));
                }
                Some(Citation::UserNote { block_id, .. }) => println!("{mark} {} (tu nota {block_id})", b.text),
                None => println!("{mark} {}  [SIN CITA]", b.text),
            }
        }
        println!();
    }
    if let Some(out) = a.get(2) {
        std::fs::write(out, serde_json::to_string_pretty(&notes)?)?;
    }
    Ok(())
}
