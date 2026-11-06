//! Plantillas de prompts versionadas: enhance, chat, recipe, brief, follow-up,
//! dictation-cleanup, title. Se cargan desde `templates/*.md` en fase 1.

pub const ENHANCE_SYSTEM: &str = include_str!("../templates/enhance.system.md");
pub const CHAT_SYSTEM: &str = include_str!("../templates/chat.system.md");
pub const DICTATION_CLEANUP: &str = include_str!("../templates/dictation.cleanup.md");
