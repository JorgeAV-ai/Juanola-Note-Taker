//! Comandos expuestos al frontend. En fase 1 se migran a tauri-specta para
//! generar los tipos TypeScript automáticamente.

use std::sync::Arc;

use juanola_core_types::{CalendarEvent, Id, Note, NoteStatus, Segment};
use juanola_db::repo;
use juanola_enhance::pipelines::enhance::enhance;
use juanola_enhance::{EnhanceInput, EnhancedNotes};
use juanola_llm::backends::llamacpp::LlamaCpp;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::recording::Active;
use crate::state::AppState;

type Res<T> = Result<T, String>;

fn e(err: impl std::fmt::Display) -> String {
    err.to_string()
}

#[derive(Serialize)]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo { version: env!("CARGO_PKG_VERSION").to_string(), data_dir: state.data_dir.display().to_string() }
}

// ------------------------------------------------------------------- notas

#[tauri::command]
pub async fn list_notes(state: State<'_, AppState>) -> Res<Vec<Note>> {
    repo::notes::list_recent(&state.db, 200).await.map_err(e)
}

#[tauri::command]
pub async fn create_note(state: State<'_, AppState>, title: Option<String>) -> Res<Note> {
    let ws = repo::workspaces::ensure_personal(&state.db).await.map_err(e)?;
    repo::notes::create(&state.db, ws, title.as_deref().unwrap_or("")).await.map_err(e)
}

#[tauri::command]
pub async fn get_note(state: State<'_, AppState>, id: Id) -> Res<Option<Note>> {
    repo::notes::get(&state.db, id).await.map_err(e)
}

#[tauri::command]
pub async fn set_note_title(state: State<'_, AppState>, id: Id, title: String) -> Res<()> {
    repo::notes::set_title(&state.db, id, &title).await.map_err(e)
}

#[tauri::command]
pub async fn set_note_raw_doc(state: State<'_, AppState>, id: Id, doc: serde_json::Value) -> Res<()> {
    repo::notes::set_raw_doc(&state.db, id, &doc).await.map_err(e)
}

#[tauri::command]
pub async fn list_segments(state: State<'_, AppState>, note_id: Id) -> Res<Vec<Segment>> {
    repo::segments::list(&state.db, note_id).await.map_err(e)
}

// --------------------------------------------------------------- grabación

#[tauri::command]
pub fn start_recording(app: AppHandle, state: State<'_, AppState>, note_id: Id, language: Option<String>) -> Res<()> {
    let mut slot = state.recording.lock().map_err(e)?;
    if let Some(active) = slot.as_ref() {
        return Err(format!("ya hay una grabación en curso (nota {})", active.note_id));
    }
    let language = language.or_else(|| Some("Spanish".into())).filter(|l| l != "auto");
    *slot = Some(Active::start(app, state.db.clone(), state.paths(), note_id, language)?);
    Ok(())
}

#[tauri::command]
pub fn pause_recording(state: State<'_, AppState>) -> Res<()> {
    if let Some(a) = state.recording.lock().map_err(e)?.as_ref() {
        a.pause();
    }
    Ok(())
}

#[tauri::command]
pub fn resume_recording(state: State<'_, AppState>) -> Res<()> {
    if let Some(a) = state.recording.lock().map_err(e)?.as_ref() {
        a.resume();
    }
    Ok(())
}

/// Para la grabación y espera a los últimos segmentos. Devuelve cuántos se guardaron.
#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> Res<usize> {
    let active = state.recording.lock().map_err(e)?.take();
    let Some(active) = active else {
        tracing::warn!("stop_recording sin grabación activa");
        return Ok(0);
    };
    tracing::info!(note_id = %active.note_id, "stop_recording");
    tauri::async_runtime::spawn_blocking(move || active.stop()).await.map_err(e)?
}

// ------------------------------------------------------------------ enhance

#[derive(Serialize, Clone)]
struct NoteStatusEvent {
    note_id: Id,
    status: NoteStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

async fn llm(state: &AppState) -> Res<Arc<LlamaCpp>> {
    let path = state.gguf();
    state
        .llm
        .get_or_try_init(|| async move {
            tauri::async_runtime::spawn_blocking(move || LlamaCpp::load(&path, 32_768).map(Arc::new))
                .await
                .map_err(e)?
                .map_err(e)
        })
        .await
        .cloned()
}

/// Genera las notas mejoradas de una nota a partir de sus notas crudas y su
/// transcripción, y las guarda en `enhanced_doc`.
#[tauri::command]
pub async fn enhance_note(app: AppHandle, state: State<'_, AppState>, id: Id) -> Res<EnhancedNotes> {
    let set = |status: NoteStatus, message: Option<String>| {
        let _ = app.emit("note://status", NoteStatusEvent { note_id: id, status, message });
    };
    repo::notes::set_status(&state.db, id, NoteStatus::Enhancing).await.map_err(e)?;
    set(NoteStatus::Enhancing, None);
    let result = async {
        let note = repo::notes::get(&state.db, id).await.map_err(e)?.ok_or("la nota no existe")?;
        let segments = repo::segments::list(&state.db, id).await.map_err(e)?;
        let model = llm(&state).await?;
        let input = EnhanceInput { note: &note, segments: &segments, template: None, summary_language: "español", user_profile: "" };
        let (notes, report) = enhance(model.as_ref(), &input).await.map_err(e)?;
        tracing::info!(?report, "enhance");
        let value = serde_json::to_value(&notes).map_err(e)?;
        repo::notes::set_enhanced_doc(&state.db, id, Some(&value)).await.map_err(e)?;
        Ok::<_, String>(notes)
    }
    .await;
    match &result {
        Ok(_) => {
            repo::notes::set_status(&state.db, id, NoteStatus::Ready).await.map_err(e)?;
            set(NoteStatus::Ready, None);
        }
        Err(msg) => {
            repo::notes::set_status(&state.db, id, NoteStatus::Failed).await.map_err(e)?;
            set(NoteStatus::Failed, Some(msg.clone()));
        }
    }
    result
}

// ------------------------------------------------------------------- otros

#[tauri::command]
pub fn list_audio_devices() -> Vec<juanola_audio_capture::AudioDevice> {
    use juanola_audio_capture::AudioCapture;
    juanola_audio_capture::CpalCapture::new().list_input_devices().unwrap_or_default()
}

#[tauri::command]
pub async fn coming_up(_state: State<'_, AppState>) -> Res<Vec<CalendarEvent>> {
    // Fase 2: juanola_calendar
    Ok(vec![])
}
