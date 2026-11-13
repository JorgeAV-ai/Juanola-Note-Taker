//! Exportar una nota a Markdown o HTML, y el resumen en Markdown para copiar.
//!
//! El documento lleva el título, la fecha, el resumen (apartados, viñetas y
//! tareas, sin las citas), las notas del usuario y la transcripción. Los
//! títulos de cada parte llegan del frontend en el idioma de la interfaz.

use std::fmt::Write as _;

use juanola_core_types::{Channel, Note, Segment};
use juanola_enhance::EnhancedNotes;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Markdown,
    Html,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Markdown => "md",
            Format::Html => "html",
        }
    }
}

/// Títulos de las partes y de los hablantes, en el idioma de la interfaz.
#[derive(Debug, Clone, Deserialize)]
pub struct Labels {
    pub untitled: String,
    pub summary: String,
    pub my_notes: String,
    pub transcript: String,
    pub me: String,
    pub them: String,
}

/// Un bloque del documento, común a los dos formatos.
#[derive(Debug, PartialEq)]
enum Block {
    Heading(u8, String),
    Para(String),
    /// Viñeta; `Some(done)` si es una tarea.
    Item(usize, Option<bool>, String),
    Line(String, String),
}

/// El resumen en Markdown, para copiarlo.
pub fn summary_markdown(notes: &EnhancedNotes) -> String {
    let mut blocks = Vec::new();
    summary_blocks(notes, 2, &mut blocks);
    render_markdown(&blocks)
}

/// La nota entera en el formato pedido.
pub fn document(note: &Note, enhanced: Option<&EnhancedNotes>, segments: &[Segment], labels: &Labels, format: Format) -> String {
    let title = if note.title.trim().is_empty() { labels.untitled.clone() } else { note.title.trim().to_string() };
    let mut blocks = vec![Block::Para(note.created_at.format("%Y-%m-%d %H:%M UTC").to_string())];
    if let Some(e) = enhanced.filter(|e| !e.sections.is_empty()) {
        blocks.push(Block::Heading(2, labels.summary.clone()));
        summary_blocks(e, 3, &mut blocks);
    }
    let mut mine = Vec::new();
    prosemirror_blocks(&note.raw_doc, 0, &mut mine);
    if !mine.is_empty() {
        blocks.push(Block::Heading(2, labels.my_notes.clone()));
        blocks.extend(mine);
    }
    let finals: Vec<&Segment> = segments.iter().filter(|s| s.is_final && !s.text.trim().is_empty()).collect();
    if !finals.is_empty() {
        blocks.push(Block::Heading(2, labels.transcript.clone()));
        for s in finals {
            let who = if s.channel == Channel::Mic { &labels.me } else { &labels.them };
            blocks.push(Block::Line(format!("{who} · {}", clock(s.start)), s.text.trim().to_string()));
        }
    }
    match format {
        Format::Markdown => format!("# {}\n\n{}", title, render_markdown(&blocks)),
        Format::Html => render_html(&title, &blocks),
    }
}

/// Nombre de archivo propuesto: el título sin caracteres problemáticos.
pub fn file_name(note: &Note, labels: &Labels, format: Format) -> String {
    let base: String = note
        .title
        .trim()
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '-' } else { c })
        .collect();
    let base = if base.trim().is_empty() { labels.untitled.clone() } else { base.trim().chars().take(80).collect() };
    format!("{base}.{}", format.extension())
}

fn summary_blocks(notes: &EnhancedNotes, level: u8, out: &mut Vec<Block>) {
    for s in &notes.sections {
        if !s.heading.trim().is_empty() {
            out.push(Block::Heading(level, s.heading.trim().to_string()));
        }
        for b in &s.bullets {
            out.push(Block::Item(0, b.is_action_item.then_some(b.done), b.text.trim().to_string()));
        }
    }
}

/// Notas del usuario: el documento de TipTap (párrafos, títulos, listas,
/// tareas). Las imágenes se omiten.
fn prosemirror_blocks(node: &Value, depth: usize, out: &mut Vec<Block>) {
    let kind = node.get("type").and_then(Value::as_str).unwrap_or("");
    let children = node.get("content").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    match kind {
        "paragraph" => {
            let t = inline_text(node);
            if !t.trim().is_empty() {
                out.push(Block::Para(t));
            }
        }
        "heading" => {
            let level = node.pointer("/attrs/level").and_then(Value::as_u64).unwrap_or(2).clamp(1, 3) as u8;
            out.push(Block::Heading(level + 2, inline_text(node)));
        }
        "bulletList" | "orderedList" | "taskList" => {
            for item in children {
                let done = (item.get("type").and_then(Value::as_str) == Some("taskItem"))
                    .then(|| item.pointer("/attrs/checked").and_then(Value::as_bool).unwrap_or(false));
                let parts = item.get("content").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
                // Primer párrafo: el texto de la viñeta; el resto (sublistas), anidado.
                let text = parts.first().map(inline_text).unwrap_or_default();
                out.push(Block::Item(depth, done, text));
                for p in parts.iter().skip(1) {
                    prosemirror_blocks(p, depth + 1, out);
                }
            }
        }
        _ => {
            for c in children {
                prosemirror_blocks(c, depth, out);
            }
        }
    }
}

fn inline_text(node: &Value) -> String {
    let mut s = String::new();
    for c in node.get("content").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        match c.get("type").and_then(Value::as_str) {
            Some("text") => s.push_str(c.get("text").and_then(Value::as_str).unwrap_or("")),
            Some("hardBreak") => s.push('\n'),
            _ => s.push_str(&inline_text(c)),
        }
    }
    s
}

fn clock(secs: f64) -> String {
    let t = secs.max(0.0) as u64;
    format!("{:02}:{:02}", t / 60, t % 60)
}

fn render_markdown(blocks: &[Block]) -> String {
    let mut out = String::new();
    let mut prev_item = false;
    for b in blocks {
        let is_item = matches!(b, Block::Item(..));
        if !out.is_empty() && !(is_item && prev_item) {
            out.push('\n');
        }
        match b {
            Block::Heading(l, t) => {
                let _ = writeln!(out, "{} {t}", "#".repeat(*l as usize));
            }
            Block::Para(t) => {
                let _ = writeln!(out, "{}", t.replace('\n', "  \n"));
            }
            Block::Item(d, task, t) => {
                let check = match task {
                    Some(true) => "[x] ",
                    Some(false) => "[ ] ",
                    None => "",
                };
                let _ = writeln!(out, "{}- {check}{t}", "  ".repeat(*d));
            }
            Block::Line(who, t) => {
                let _ = writeln!(out, "**{who}** {t}");
            }
        }
        prev_item = is_item;
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn render_html(title: &str, blocks: &[Block]) -> String {
    let mut body = String::new();
    let mut open_list = false;
    for b in blocks {
        let is_item = matches!(b, Block::Item(..));
        if open_list && !is_item {
            body.push_str("</ul>\n");
            open_list = false;
        }
        match b {
            Block::Heading(l, t) => {
                let _ = writeln!(body, "<h{l}>{}</h{l}>", esc(t));
            }
            Block::Para(t) => {
                let _ = writeln!(body, "<p>{}</p>", esc(t).replace('\n', "<br>"));
            }
            Block::Item(d, task, t) => {
                if !open_list {
                    body.push_str("<ul>\n");
                    open_list = true;
                }
                let check = match task {
                    Some(true) => "<input type=\"checkbox\" checked disabled> ",
                    Some(false) => "<input type=\"checkbox\" disabled> ",
                    None => "",
                };
                let style = if *d > 0 { format!(" style=\"margin-left:{}em\"", d * 2) } else { String::new() };
                let _ = writeln!(body, "<li{style}>{check}{}</li>", esc(t));
            }
            Block::Line(who, t) => {
                let _ = writeln!(body, "<p class=\"line\"><b>{}</b> {}</p>", esc(who), esc(t));
            }
        }
    }
    if open_list {
        body.push_str("</ul>\n");
    }
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{t}</title>\n<style>body{{font:16px/1.6 -apple-system,system-ui,sans-serif;max-width:720px;margin:40px auto;padding:0 16px;color:#1d1b18}}h1{{font-weight:500}}h2{{margin-top:2em;border-bottom:1px solid #e6e1d6;padding-bottom:4px}}ul{{padding-left:1.2em}}li{{margin:2px 0}}.line{{margin:4px 0}}.line b{{color:#6b665c;font-weight:600}}</style>\n</head><body>\n<h1>{t}</h1>\n{body}</body></html>\n",
        t = esc(title)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use juanola_enhance::{Bullet, Section};
    use serde_json::json;

    fn labels() -> Labels {
        Labels {
            untitled: "Sin título".into(),
            summary: "Resumen".into(),
            my_notes: "Mis notas".into(),
            transcript: "Transcripción".into(),
            me: "Yo".into(),
            them: "Ellos".into(),
        }
    }

    fn bullet(text: &str, action: bool, done: bool) -> Bullet {
        Bullet { text: text.into(), is_user_text: false, source: None, is_action_item: action, done }
    }

    fn enhanced() -> EnhancedNotes {
        EnhancedNotes {
            sections: vec![Section {
                heading: "Decisiones".into(),
                bullets: vec![bullet("Lanzar el <lunes>", false, false), bullet("Laura prepara la demo", true, true)],
            }],
            edited: false,
        }
    }

    fn note() -> Note {
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-13T10:30:00Z").unwrap().with_timezone(&chrono::Utc);
        Note {
            id: juanola_core_types::Id::nil(),
            workspace_id: juanola_core_types::Id::nil(),
            title: "Semanal: producto".into(),
            raw_doc: json!({"type":"doc","content":[
                {"type":"paragraph","content":[{"type":"text","text":"Idea "},{"type":"text","text":"suelta","marks":[{"type":"bold"}]}]},
                {"type":"taskList","content":[{"type":"taskItem","attrs":{"checked":false},"content":[{"type":"paragraph","content":[{"type":"text","text":"Llamar a Ana"}]}]}]},
                {"type":"paragraph"}
            ]}),
            enhanced_doc: None,
            template_id: None,
            status: juanola_core_types::NoteStatus::Ready,
            calendar_event_id: None,
            created_at: now,
            updated_at: now,
            trashed_at: None,
        }
    }

    fn seg(channel: Channel, start: f64, text: &str, is_final: bool) -> Segment {
        Segment {
            id: juanola_core_types::Id::nil(),
            note_id: juanola_core_types::Id::nil(),
            channel,
            speaker_id: None,
            start,
            end: start + 2.0,
            text: text.into(),
            confidence: None,
            is_final,
        }
    }

    #[test]
    fn summary_markdown_with_tasks() {
        assert_eq!(summary_markdown(&enhanced()), "## Decisiones\n\n- Lanzar el <lunes>\n- [x] Laura prepara la demo\n");
    }

    #[test]
    fn markdown_document_has_all_parts() {
        let segs = [
            seg(Channel::System, 65.0, "Empezamos.", true),
            seg(Channel::Mic, 70.0, "a medias", false),
            seg(Channel::Mic, 72.0, "Vale.", true),
        ];
        let md = document(&note(), Some(&enhanced()), &segs, &labels(), Format::Markdown);
        assert_eq!(
            md,
            "# Semanal: producto\n\n2026-11-13 10:30 UTC\n\n## Resumen\n\n### Decisiones\n\n- Lanzar el <lunes>\n- [x] Laura prepara la demo\n\n## Mis notas\n\nIdea suelta\n\n- [ ] Llamar a Ana\n\n## Transcripción\n\n**Ellos · 01:05** Empezamos.\n\n**Yo · 01:12** Vale.\n"
        );
    }

    #[test]
    fn html_escapes_and_lists() {
        let html = document(&note(), Some(&enhanced()), &[], &labels(), Format::Html);
        assert!(html.contains("<title>Semanal: producto</title>"));
        assert!(html.contains("<li>Lanzar el &lt;lunes&gt;</li>"));
        assert!(html.contains("<li><input type=\"checkbox\" checked disabled> Laura prepara la demo</li>"));
        assert!(!html.contains("Transcripción"));
        assert_eq!(html.matches("<ul>").count(), html.matches("</ul>").count());
    }

    #[test]
    fn empty_note_and_file_name() {
        let mut n = note();
        n.title = "  ".into();
        n.raw_doc = json!({"type":"doc","content":[{"type":"paragraph"}]});
        assert_eq!(document(&n, None, &[], &labels(), Format::Markdown), "# Sin título\n\n2026-11-13 10:30 UTC\n");
        assert_eq!(file_name(&n, &labels(), Format::Html), "Sin título.html");
        assert_eq!(file_name(&note(), &labels(), Format::Markdown), "Semanal- producto.md");
    }
}
