//! Modelos de dominio compartidos por todos los crates.
//! Reflejan uno a uno los conceptos de Granola: Workspace, Space, Folder, Note,
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
    /// Subcarpetas: un solo nivel, como en Granola.
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
    /// Las tres partes que documenta Granola.
    pub purpose: String,
    pub style: String,
    pub structure: String,
    pub is_builtin: bool,
    pub shared_with_workspace: bool,
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
