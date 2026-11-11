//! Modelos de dominio compartidos por todos los crates.
//! Conceptos: Workspace, Space, Folder, Note,
//! Transcript/Segment, Person, Company, Template, Recipe, Chat.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Id = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: Id,
    pub name: String,
    pub is_personal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceKind {
    /// "My notes": privado del usuario.
    MyNotes,
    /// "Team space": visible para todo el workspace.
    Team,
    /// Espacios creados por admins (departamentos, regiones...).
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    pub id: Id,
    pub workspace_id: Id,
    pub kind: SpaceKind,
    pub name: String,
    pub icon: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: Id,
    pub space_id: Id,
    /// Subcarpetas: un solo nivel.
    pub parent_id: Option<Id>,
    pub name: String,
    pub icon: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteStatus {
    Draft,
    Transcribing,
    Enhancing,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: Id,
    pub workspace_id: Id,
    pub title: String,
    /// Notas crudas del usuario (documento ProseMirror serializado).
    pub raw_doc: serde_json::Value,
    /// Notas mejoradas por la IA (ProseMirror con marcas `ai-text` y nodos `transcript-ref`).
    pub enhanced_doc: Option<serde_json::Value>,
    pub template_id: Option<Id>,
    pub status: NoteStatus,
    pub calendar_event_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub trashed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// Micrófono: burbuja verde a la derecha ("Me").
    Mic,
    /// Audio del sistema: burbuja gris a la izquierda ("Them").
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Speaker {
    pub id: Id,
    /// Nombre resuelto por speaker tags (Accesibilidad) o asignado por el usuario.
    pub display_name: Option<String>,
    pub person_id: Option<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: Id,
    pub note_id: Id,
    pub channel: Channel,
    pub speaker_id: Option<Id>,
    /// Segundos desde el inicio de la grabación.
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub confidence: Option<f32>,
    /// `false` mientras el STT en streaming aún puede revisar el texto.
    pub is_final: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: Id,
    pub email: String,
    pub name: Option<String>,
    pub title: Option<String>,
    pub avatar_url: Option<String>,
    pub company_id: Option<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub id: Id,
    pub domain: String,
    pub name: Option<String>,
    pub logo_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: Id,
    pub name: String,
    /// Tres partes: propósito, estilo y secciones.
    pub purpose: String,
    pub style: String,
    /// Apartados de las notas, en orden. En la base de datos se guarda como
    /// JSON en la columna `structure TEXT`; ver [`parse_structure`].
    pub structure: Vec<TemplateSection>,
    pub is_builtin: bool,
    pub shared_with_workspace: bool,
}

/// Un apartado de una plantilla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateSection {
    /// Título exacto que se muestra.
    pub heading: String,
    /// Qué va aquí; solo se usa en el prompt y para repartir viñetas.
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub kind: SectionKind,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    /// Temas: viñetas con ideas, decisiones y cifras.
    #[default]
    Topics,
    /// Tareas: cada viñeta es un `is_action_item`.
    Tasks,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TemplateError {
    #[error("la plantilla necesita de 1 a {MAX_TEMPLATE_SECTIONS} apartados (tiene {0})")]
    SectionCount(usize),
    #[error("el título del apartado {0} debe tener de 1 a {MAX_HEADING_CHARS} caracteres")]
    HeadingLength(usize),
    #[error("el título «{0}» no puede llevar «#» ni saltos de línea")]
    HeadingChars(String),
    #[error("el título «{0}» está repetido")]
    DuplicateHeading(String),
    #[error("solo puede haber un apartado de tareas")]
    TooManyTasks,
    #[error("la descripción de «{0}» pasa de {MAX_HINT_CHARS} caracteres o tiene saltos de línea")]
    HintLength(String),
}

pub const MAX_TEMPLATE_SECTIONS: usize = 10;
pub const MAX_HEADING_CHARS: usize = 60;
pub const MAX_HINT_CHARS: usize = 200;

impl Template {
    /// Reglas para guardar una plantilla y antes de usarla en el enhance.
    pub fn validate(&self) -> Result<(), TemplateError> {
        let n = self.structure.len();
        if n == 0 || n > MAX_TEMPLATE_SECTIONS {
            return Err(TemplateError::SectionCount(n));
        }
        let mut seen = std::collections::HashSet::new();
        for (i, s) in self.structure.iter().enumerate() {
            let len = s.heading.trim().chars().count();
            if len == 0 || len > MAX_HEADING_CHARS {
                return Err(TemplateError::HeadingLength(i + 1));
            }
            if s.heading.contains(['#', '\n', '\r']) {
                return Err(TemplateError::HeadingChars(s.heading.clone()));
            }
            if !seen.insert(fold(&s.heading)) {
                return Err(TemplateError::DuplicateHeading(s.heading.clone()));
            }
            if s.hint.chars().count() > MAX_HINT_CHARS || s.hint.contains(['\n', '\r']) {
                return Err(TemplateError::HintLength(s.heading.clone()));
            }
        }
        if self.structure.iter().filter(|s| s.kind == SectionKind::Tasks).count() > 1 {
            return Err(TemplateError::TooManyTasks);
        }
        Ok(())
    }
}

/// Lee la columna `structure`: JSON de `Vec<TemplateSection>` o, en el formato
/// antiguo de texto libre, un apartado `Topics` por cada línea no vacía.
pub fn parse_structure(text: &str) -> Vec<TemplateSection> {
    if let Ok(s) = serde_json::from_str::<Vec<TemplateSection>>(text) {
        return s;
    }
    text.lines()
        .map(|l| l.trim().trim_start_matches(['#', '-', '*', ' ']).trim())
        .filter(|l| !l.is_empty())
        .map(|l| TemplateSection { heading: l.to_owned(), hint: String::new(), kind: SectionKind::Topics })
        .collect()
}

/// Texto para comparar títulos: minúsculas, sin tildes ni puntuación y con los
/// espacios normalizados («Próximos pasos:» y «proximos  pasos» son iguales).
pub fn fold(s: &str) -> String {
    let mapped: String = s
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            'ß' => 's',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod template_tests {
    use super::*;

    fn tpl(sections: &[(&str, &str, SectionKind)]) -> Template {
        Template {
            id: Id::nil(),
            name: "t".into(),
            purpose: String::new(),
            style: String::new(),
            structure: sections
                .iter()
                .map(|(h, hint, k)| TemplateSection { heading: (*h).into(), hint: (*hint).into(), kind: *k })
                .collect(),
            is_builtin: false,
            shared_with_workspace: false,
        }
    }
    use SectionKind::*;

    #[test]
    fn validate_rules() {
        assert!(tpl(&[("Temas", "", Topics), ("Próximos pasos", "", Tasks)]).validate().is_ok());
        assert_eq!(tpl(&[]).validate(), Err(TemplateError::SectionCount(0)));
        let many: Vec<(&str, &str, SectionKind)> =
            ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"].iter().map(|h| (*h, "", Topics)).collect();
        assert_eq!(tpl(&many).validate(), Err(TemplateError::SectionCount(11)));
        assert_eq!(tpl(&[("  ", "", Topics)]).validate(), Err(TemplateError::HeadingLength(1)));
        let long = "x".repeat(61);
        assert_eq!(tpl(&[(&long, "", Topics)]).validate(), Err(TemplateError::HeadingLength(1)));
        assert!(matches!(tpl(&[("## Temas", "", Topics)]).validate(), Err(TemplateError::HeadingChars(_))));
        assert!(matches!(tpl(&[("Te\nmas", "", Topics)]).validate(), Err(TemplateError::HeadingChars(_))));
        assert!(matches!(
            tpl(&[("Decisiones", "", Topics), ("decisiónes", "", Topics)]).validate(),
            Err(TemplateError::DuplicateHeading(_))
        ));
        assert_eq!(tpl(&[("A", "", Tasks), ("B", "", Tasks)]).validate(), Err(TemplateError::TooManyTasks));
        let hint = "x".repeat(201);
        assert!(matches!(tpl(&[("A", &hint, Topics)]).validate(), Err(TemplateError::HintLength(_))));
    }

    #[test]
    fn structure_reads_json_and_old_text() {
        let s = vec![TemplateSection { heading: "Bloqueos".into(), hint: "qué frena".into(), kind: Topics }];
        assert_eq!(parse_structure(&serde_json::to_string(&s).unwrap()), s);
        let old = parse_structure("## Resumen\n\n- Decisiones\nPróximos pasos\n");
        let h: Vec<&str> = old.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(h, ["Resumen", "Decisiones", "Próximos pasos"]);
        assert!(old.iter().all(|s| s.kind == Topics && s.hint.is_empty()));
        // JSON sin `hint` ni `kind`.
        assert_eq!(parse_structure(r#"[{"heading":"A"}]"#)[0].kind, Topics);
    }

    #[test]
    fn fold_ignores_case_accents_and_punctuation() {
        assert_eq!(fold("Próximos  pasos:"), "proximos pasos");
        assert_eq!(fold("¿Cómo está?"), "como esta");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipeScope {
    SingleMeeting,
    MultipleMeetings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: Id,
    /// Es el atajo que se escribe tras `/` en el chat.
    pub name: String,
    pub prompt: String,
    pub scope: RecipeScope,
    pub model_slot: Option<String>,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub citations: Vec<Citation>,
}

/// Cita de una respuesta de chat o de un bullet de notas mejoradas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Citation {
    UserNote { note_id: Id, block_id: String },
    Transcript { note_id: Id, start: f64, end: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub provider: String,
    pub calendar_id: String,
    pub title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub attendees: Vec<Attendee>,
    pub conference_url: Option<String>,
    pub recurring_event_id: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attendee {
    pub email: String,
    pub name: Option<String>,
    pub declined: bool,
    pub is_self: bool,
}
