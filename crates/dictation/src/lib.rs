//! Voice Typing: hotkey global (fn / Ctrl+Win), mantener para hablar, doble toque
//! para manos libres (máx. 20 min), Esc cancela, limpieza con LLM pequeño,
//! pegado en el cursor y contexto de la app destino.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DictationState {
    Idle,
    Listening,
    HandsFree,
    Finishing,
}

pub const HANDS_FREE_MAX_SECS: u64 = 20 * 60;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DictationSettings {
    pub enabled: bool,
    pub shortcut: String,
    pub languages: Vec<String>,
    pub microphone: Option<String>,
    pub mute_other_audio: bool,
    pub screen_context: bool,
}
