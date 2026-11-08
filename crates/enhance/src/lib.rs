//! Pipelines de IA: enhance notes, templates, chat RAG, recipes, rewrite,
//! briefs, follow-up emails, live chat ("What did I miss?").

use juanola_core_types::{Citation, Note, Segment, Template};
use serde::{Deserialize, Serialize};

/// Salida estructurada del enhance; el frontend la convierte a ProseMirror
/// pintando `is_user_text` en negro y el resto en gris con lupa.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedNotes {
    pub sections: Vec<Section>,
    /// El usuario las ha editado: regenerarlas perdería esos cambios.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub edited: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub heading: String,
    pub bullets: Vec<Bullet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bullet {
    pub text: String,
    pub is_user_text: bool,
    pub source: Option<Citation>,
    pub is_action_item: bool,
    /// Tarea marcada como hecha en el editor (la IA siempre la crea sin marcar).
    #[serde(default)]
    pub done: bool,
}

pub struct EnhanceInput<'a> {
    pub note: &'a Note,
    pub segments: &'a [Segment],
    pub template: Option<&'a Template>,
    pub summary_language: &'a str,
    pub user_profile: &'a str,
}

pub fn enhanced_notes_schema() -> serde_json::Value {
    serde_json::json!({
      "type": "object",
      "required": ["sections"],
      "properties": {
        "sections": {"type": "array", "items": {
          "type": "object", "required": ["heading", "bullets"],
          "properties": {
            "heading": {"type": "string"},
            "bullets": {"type": "array", "items": {
              "type": "object", "required": ["text", "is_user_text", "is_action_item"],
              "properties": {
                "text": {"type": "string"},
                "is_user_text": {"type": "boolean"},
                "is_action_item": {"type": "boolean"},
                "source": {"type": ["object", "null"]}
              }
            }}
          }
        }}
      }
    })
}

pub mod itn;
pub mod language;
pub mod pipelines;
