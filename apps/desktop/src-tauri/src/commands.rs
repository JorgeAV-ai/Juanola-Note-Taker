//! Comandos expuestos al frontend. En fase 1 se migran a tauri-specta para
//! generar los tipos TypeScript automáticamente.

use crate::state::AppState;
use granola_core_types::{CalendarEvent, Note};
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: state.data_dir.display().to_string(),
    }
}

#[tauri::command]
pub async fn list_notes(_state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    // Fase 1: repo::notes::list_recent(&state.db)
    Ok(vec![])
}

#[tauri::command]
pub async fn create_note(_state: State<'_, AppState>, title: Option<String>) -> Result<Note, String> {
    let now = chrono_now();
    Ok(Note {
        id: uuid::Uuid::new_v4(),
        workspace_id: uuid::Uuid::nil(),
        title: title.unwrap_or_default(),
        raw_doc: serde_json::json!({"type": "doc", "content": []}),
        enhanced_doc: None,
        template_id: None,
        status: granola_core_types::NoteStatus::Draft,
        calendar_event_id: None,
        created_at: now,
        updated_at: now,
        trashed_at: None,
    })
}

#[tauri::command]
pub fn list_audio_devices() -> Vec<granola_audio_capture::AudioDevice> {
    // Fase 1: backend cpal
    vec![]
}

#[tauri::command]
pub async fn coming_up(_state: State<'_, AppState>) -> Result<Vec<CalendarEvent>, String> {
    // Fase 2: granola_calendar
    Ok(vec![])
}

fn chrono_now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}
