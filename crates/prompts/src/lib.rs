//! Plantillas de prompts versionadas: enhance, chat, recipe, brief, follow-up,
//! dictation-cleanup, title. Se cargan desde `templates/*.md` en fase 1.

/// Prompt de usuario del enhance (instrucciones después de la transcripción:
/// los modelos pequeños siguen mejor lo último que leen). Variables:
/// `{{context}}`, `{{transcript}}`, `{{notes}}`, `{{language}}`, `{{sections}}`.
pub const ENHANCE_USER: &str = include_str!("../templates/enhance.user.md");
/// Extracción de datos concretos de un fragmento (pasada de detalle del
/// enhance). Variables: `{{from}}`, `{{to}}`, `{{transcript}}`, `{{language}}`.
pub const ENHANCE_DETAILS: &str = include_str!("../templates/enhance.details.md");
pub const CHAT_SYSTEM: &str = include_str!("../templates/chat.system.md");
pub const DICTATION_CLEANUP: &str = include_str!("../templates/dictation.cleanup.md");
