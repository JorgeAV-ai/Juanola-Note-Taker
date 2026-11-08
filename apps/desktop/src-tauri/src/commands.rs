//! Comandos expuestos al frontend. En fase 1 se migran a tauri-specta para
//! generar los tipos TypeScript automáticamente.

use std::sync::Arc;

use juanola_core_types::{CalendarEvent, Id, Note, NoteStatus, Segment};
use juanola_db::repo;
use juanola_db::repo::folders::Folder;
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

// ----------------------------------------------------------------- modelos

#[derive(Serialize)]
pub struct ModelsState {
    models: Vec<juanola_models::install::ModelStatus>,
    installing: bool,
}

#[tauri::command]
pub fn models_status(state: State<'_, AppState>) -> Res<ModelsState> {
    Ok(ModelsState { models: state.models.status()?, installing: state.models.installing() })
}

/// Empieza a descargar los modelos que falten (progreso en `models://progress`).
#[tauri::command]
pub fn install_models(app: AppHandle, state: State<'_, AppState>) {
    state.models.start_install(app);
}

#[tauri::command]
pub fn cancel_model_install(state: State<'_, AppState>) {
    state.models.cancel();
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

/// Guarda las notas mejoradas tras editarlas a mano. Llegan ya convertidas a
/// `EnhancedNotes` desde el editor; deserializarlas aquí las valida.
#[tauri::command]
pub async fn set_note_enhanced_doc(state: State<'_, AppState>, id: Id, doc: EnhancedNotes) -> Res<()> {
    let value = serde_json::to_value(&doc).map_err(e)?;
    repo::notes::set_enhanced_doc(&state.db, id, Some(&value)).await.map_err(e)
}

// ----------------------------------------------------- papelera y carpetas

#[tauri::command]
pub async fn trash_note(state: State<'_, AppState>, id: Id) -> Res<()> {
    let recording = state.recording.lock().map_err(e)?.as_ref().is_some_and(|a| a.note_id == id);
    if recording {
        return Err("Para la grabación antes de borrar la nota.".into());
    }
    repo::notes::trash(&state.db, id).await.map_err(e)
}

#[tauri::command]
pub async fn restore_note(state: State<'_, AppState>, id: Id) -> Res<()> {
    repo::notes::restore(&state.db, id).await.map_err(e)
}

#[tauri::command]
pub async fn list_trashed_notes(state: State<'_, AppState>) -> Res<Vec<Note>> {
    repo::notes::list_trashed(&state.db).await.map_err(e)
}

/// Solo borra notas que ya están en la papelera.
#[tauri::command]
pub async fn delete_note_forever(state: State<'_, AppState>, id: Id) -> Res<()> {
    repo::notes::delete_forever(&state.db, id).await.map_err(e)
}

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> Res<Vec<Folder>> {
    let ws = repo::workspaces::ensure_personal(&state.db).await.map_err(e)?;
    repo::folders::list(&state.db, ws).await.map_err(e)
}

#[tauri::command]
pub async fn create_folder(state: State<'_, AppState>, name: String) -> Res<Folder> {
    let name = name.trim();
    if name.is_empty() {
        return Err("la carpeta necesita un nombre".into());
    }
    let ws = repo::workspaces::ensure_personal(&state.db).await.map_err(e)?;
    repo::folders::create(&state.db, ws, name).await.map_err(e)
}

#[tauri::command]
pub async fn rename_folder(state: State<'_, AppState>, id: Id, name: String) -> Res<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err("la carpeta necesita un nombre".into());
    }
    repo::folders::rename(&state.db, id, name).await.map_err(e)
}

#[tauri::command]
pub async fn delete_folder(state: State<'_, AppState>, id: Id) -> Res<()> {
    repo::folders::delete(&state.db, id).await.map_err(e)
}

#[tauri::command]
pub async fn list_folder_notes(state: State<'_, AppState>, id: Id) -> Res<Vec<Note>> {
    repo::folders::notes(&state.db, id).await.map_err(e)
}

#[tauri::command]
pub async fn get_note_folder(state: State<'_, AppState>, note_id: Id) -> Res<Option<Id>> {
    repo::folders::note_folder(&state.db, note_id).await.map_err(e)
}

#[tauri::command]
pub async fn set_note_folder(state: State<'_, AppState>, note_id: Id, folder_id: Option<Id>) -> Res<()> {
    repo::folders::set_note_folder(&state.db, note_id, folder_id).await.map_err(e)
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
    state.models.require(&["vad", "stt_streaming"])?;
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
    let mut slot = state.llm.lock().await;
    if let Some(m) = slot.as_ref() {
        return Ok(m.clone());
    }
    let path = state.gguf();
    let model = tauri::async_runtime::spawn_blocking(move || LlamaCpp::load(&path, 32_768).map(Arc::new))
        .await
        .map_err(e)?
        .map_err(e)?;
    *slot = Some(model.clone());
    Ok(model)
}

/// Genera las notas mejoradas de una nota a partir de sus notas crudas y su
/// transcripción, y las guarda en `enhanced_doc`.
#[tauri::command]
pub async fn enhance_note(app: AppHandle, state: State<'_, AppState>, id: Id) -> Res<EnhancedNotes> {
    let set = |status: NoteStatus, message: Option<String>| {
        let _ = app.emit("note://status", NoteStatusEvent { note_id: id, status, message });
    };
    state.models.require(&["llm_main"])?;
    repo::notes::set_status(&state.db, id, NoteStatus::Enhancing).await.map_err(e)?;
    set(NoteStatus::Enhancing, None);
    let result = async {
        let note = repo::notes::get(&state.db, id).await.map_err(e)?.ok_or("la nota no existe")?;
        let segments = repo::segments::list(&state.db, id).await.map_err(e)?;
        let model = llm(&state).await?;
        let summary_language = juanola_enhance::language::summary_language(&segments);
        tracing::info!(summary_language, "idioma del resumen");
        let input = EnhanceInput { note: &note, segments: &segments, template: None, summary_language, user_profile: "" };
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
            tracing::error!(note_id = %id, error = %msg, "enhance falló");
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

/// Abre el panel de privacidad de Ajustes del Sistema que corresponde a `kind`
/// (`"mic_settings"` o `"audio_capture_settings"`).
#[tauri::command]
pub fn open_privacy_settings(kind: String) -> Res<()> {
    let pane = match kind.as_str() {
        "mic_settings" => "Privacy_Microphone",
        "audio_capture_settings" => "Privacy_ScreenCapture",
        other => return Err(format!("ajuste desconocido: {other}")),
    };
    std::process::Command::new("open")
        .arg(format!("x-apple.systempreferences:com.apple.preference.security?{pane}"))
        .status()
        .map_err(e)
        .and_then(|s| if s.success() { Ok(()) } else { Err(format!("open terminó con {s}")) })
}

/// Error de pintado del frontend (lo captura su ErrorBoundary): al log.
#[tauri::command]
pub fn frontend_error(message: String) {
    tracing::error!(target: "frontend", "{message}");
}

/// Estado del permiso de micrófono para la pantalla de Ajustes. (El de audio
/// del sistema no tiene API de consulta: solo se sabe al grabar.)
#[tauri::command]
pub fn mic_permission_status() -> juanola_audio_capture::Permission {
    juanola_audio_capture::mic_permission()
}

/// Abre la carpeta de datos de Juanola en Finder.
#[tauri::command]
pub fn reveal_data_dir(state: State<'_, AppState>) -> Res<()> {
    std::process::Command::new("open").arg(&state.data_dir).status().map_err(e)?;
    Ok(())
}

// --------------------------------------------------------- autotest (desarrollo)
//
// Con `JUANOLA_AUTOTEST=1` el frontend recorre solo el flujo completo pulsando
// los botones reales (ver `src/dev/autotest.ts`) y deja el resultado en el log.
// Sirve para probar la .app con permisos reales sin tocar la ventana.

fn autotest_enabled() -> bool {
    std::env::var_os("JUANOLA_AUTOTEST").is_some_and(|v| v != "0")
}

/// Cierra la app por el camino normal (como ⌘Q) al acabar el autotest, si
/// `JUANOLA_AUTOTEST_QUIT=1`: sirve para comprobar que se sale sin abortar.
#[tauri::command]
pub fn autotest_quit(app: AppHandle) {
    if autotest_enabled() && std::env::var_os("JUANOLA_AUTOTEST_QUIT").is_some() {
        tracing::info!(target: "autotest", "cerrando la app");
        app.exit(0);
    }
}

/// Modo del autotest: `None` si está apagado; `"1"` recorrido completo;
/// `"enhance"` solo enhance + editor sobre una nota ya transcrita.
#[tauri::command]
pub fn autotest_config() -> Option<String> {
    autotest_enabled().then(|| std::env::var("JUANOLA_AUTOTEST").unwrap_or_default())
}

#[tauri::command]
pub fn autotest_log(line: String) {
    tracing::info!(target: "autotest", "{line}");
}

/// Reproduce `text` con `say` por la salida del sistema y vuelve al terminar.
#[tauri::command]
pub async fn autotest_say(text: String, voice: Option<String>) -> Res<()> {
    if !autotest_enabled() {
        return Err("solo en modo autotest".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut cmd = std::process::Command::new("say");
        if let Some(v) = voice {
            cmd.arg("-v").arg(v);
        }
        cmd.arg(text).status().map_err(e).map(|_| ())
    })
    .await
    .map_err(e)?
}
