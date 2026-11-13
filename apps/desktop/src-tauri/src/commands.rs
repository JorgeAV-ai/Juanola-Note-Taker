//! Comandos expuestos al frontend. En fase 1 se migran a tauri-specta para
//! generar los tipos TypeScript automáticamente.

use std::sync::Arc;

use juanola_calendar::Access;
use juanola_core_types::{CalendarEvent, Id, Note, NoteStatus, Segment, Template};
use juanola_db::repo;
use juanola_db::repo::folders::Folder;
use juanola_enhance::pipelines::enhance::enhance;
use juanola_enhance::templates::{builtin_template, builtin_templates, is_builtin};
use juanola_enhance::{EnhanceInput, EnhancedNotes};
use juanola_llm::backends::llamacpp::LlamaCpp;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::detect::{self, DetectionSettings, Notification, NotificationKind, TakeNotes};
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
    // Bajo `lifecycle` (como `start_recording` y `stop_recording`): la nota no
    // se puede borrar mientras la validan para grabar ni mientras finaliza.
    let _lifecycle = state.lifecycle.lock().await;
    let busy = state.recording.lock().map_err(e)?.as_ref().is_some_and(|a| a.note_id == id)
        || state.stopping_note.lock().map_err(e)?.is_some_and(|n| n == id);
    if busy {
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
pub async fn start_recording(app: AppHandle, state: State<'_, AppState>, note_id: Id, language: Option<String>) -> Res<()> {
    // `lifecycle` serializa con `trash_note` y `stop_recording`: validar la
    // nota y registrar la grabación forman una sección crítica. El `Mutex`
    // del slot no puede mantenerse entre awaits (su guard no es `Send`), así
    // que solo cubre el último chequeo y la inserción.
    let _lifecycle = state.lifecycle.lock().await;
    // Grabar en una nota que no existe o está en la papelera guardaría
    // segmentos que nunca se ven (la FK los tira en silencio).
    let note = repo::notes::get(&state.db, note_id).await.map_err(e)?.ok_or("la nota no existe")?;
    if note.trashed_at.is_some() {
        return Err("la nota está en la papelera: restáurala antes de grabar".into());
    }
    state.models.require(&["vad", "stt_streaming"])?;
    let language = language.or_else(|| Some("Spanish".into())).filter(|l| l != "auto");
    let mut slot = state.recording.lock().map_err(e)?;
    if state.stopping.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("la grabación anterior aún está parando".into());
    }
    if let Some(active) = slot.as_ref() {
        return Err(format!("ya hay una grabación en curso (nota {})", active.note_id));
    }
    *slot = Some(Active::start(app, state.db.clone(), state.paths(), note_id, language)?);
    Ok(())
}

#[derive(Serialize)]
pub struct RecordingInfo {
    pub note_id: Option<Id>,
    pub stopping_note: Option<Id>,
    pub paused: bool,
    pub stopping: bool,
}

/// Estado actual de la grabación: la píldora lo consulta al montar, porque si
/// el webview se recarga en mitad de una sesión el backend sigue grabando.
#[tauri::command]
pub fn recording_state(state: State<'_, AppState>) -> RecordingInfo {
    let a = state.recording.lock().ok().and_then(|r| r.as_ref().map(|a| (a.note_id, a.is_paused())));
    RecordingInfo {
        note_id: a.map(|(id, _)| id),
        stopping_note: state.stopping_note.lock().ok().and_then(|n| *n),
        paused: a.map(|(_, p)| p).unwrap_or(false),
        stopping: state.stopping.load(std::sync::atomic::Ordering::Relaxed),
    }
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

/// Al soltarse (éxito, error o el future cancelado a mitad, p. ej. al recargar
/// el webview), libera `stopping` y `stopping_note`: si no, una cancelación
/// dejaría la grabación bloqueada para siempre.
struct StopGuard<'a> {
    stopping: &'a std::sync::atomic::AtomicBool,
    note: &'a std::sync::Mutex<Option<Id>>,
}

impl Drop for StopGuard<'_> {
    fn drop(&mut self) {
        self.stopping.store(false, std::sync::atomic::Ordering::Relaxed);
        if let Ok(mut n) = self.note.lock() {
            *n = None;
        }
    }
}

/// Para la grabación y espera a los últimos segmentos. Devuelve cuántos se guardaron.
#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> Res<usize> {
    let active = {
        // Bajo `lifecycle` (como `start_recording` y `trash_note`), y con el
        // flag y el id de la nota escritos antes de soltar el slot: ni se
        // cuela otra captura ni se borra la nota mientras llegan sus últimos
        // segmentos.
        let _lifecycle = state.lifecycle.lock().await;
        let mut slot = state.recording.lock().map_err(e)?;
        let was = state.stopping.swap(true, std::sync::atomic::Ordering::Relaxed);
        match slot.take() {
            Some(a) => {
                // `into_inner`: un mutex envenenado no debe dejar `stopping`
                // encendido sin guardia (tampoco hay awaits que lo envenenen).
                *state.stopping_note.lock().unwrap_or_else(|p| p.into_inner()) = Some(a.note_id);
                a
            }
            None => {
                // Restaurar, no apagar: otra parada puede seguir en marcha.
                state.stopping.store(was, std::sync::atomic::Ordering::Relaxed);
                if was {
                    // La parada anterior sigue en curso: avisar en vez de
                    // fingir que no había nada que parar.
                    return Err("la grabación anterior aún está parando".into());
                }
                tracing::warn!("stop_recording sin grabación activa");
                return Ok(0);
            }
        }
    };
    tracing::info!(note_id = %active.note_id, "stop_recording");
    state.detection.set_origin(detect::Origin::None);
    let _guard = StopGuard { stopping: &state.stopping, note: &state.stopping_note };
    tauri::async_runtime::spawn_blocking(move || active.stop()).await.map_err(e)?
}

// ------------------------------------------------------------------ enhance

#[derive(Serialize, Clone)]
struct NoteStatusEvent {
    note_id: Id,
    status: NoteStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    /// Aviso para el usuario, como código (lo traduce el frontend):
    /// `template_fallback` si la plantilla no encajó y se usó Auto.
    #[serde(skip_serializing_if = "Option::is_none")]
    notice: Option<&'static str>,
}

async fn llm(state: &AppState) -> Res<Arc<LlamaCpp>> {
    let mut slot = state.llm.lock().await;
    if let Some(m) = slot.as_ref() {
        return Ok(m.clone());
    }
    let path = state.gguf();
    // Flash attention forzada: en Metal recorta el coste del prefill del
    // resumen largo (llama.cpp ya la aplicaría en AUTO, pero así es explícita).
    let model = tauri::async_runtime::spawn_blocking(move || {
        LlamaCpp::load(&path, 32_768).map(|m| m.with_flash_attention(Some(true))).map(Arc::new)
    })
    .await
    .map_err(e)?
    .map_err(e)?;
    *slot = Some(model.clone());
    Ok(model)
}

/// Idioma de los títulos de las plantillas integradas: acepta el código de la
/// interfaz («es», «en») o el nombre que da `summary_language` («inglés»).
fn template_language(language: Option<&str>) -> &str {
    match language {
        Some("en") => "inglés",
        Some("es") | None => "español",
        Some(other) => other,
    }
}

/// Plantillas para el menú: las integradas (con los títulos en `language`) y
/// después las del usuario.
#[tauri::command]
pub async fn list_templates(state: State<'_, AppState>, language: Option<String>) -> Res<Vec<Template>> {
    let mut all = builtin_templates(template_language(language.as_deref()));
    all.extend(repo::templates::list(&state.db).await.map_err(e)?);
    Ok(all)
}

/// Crea o sustituye una plantilla del usuario y la devuelve como queda
/// guardada. Las integradas no se pueden editar (se duplican).
#[tauri::command]
pub async fn save_template(state: State<'_, AppState>, template: Template) -> Res<Template> {
    if is_builtin(template.id) {
        return Err("las plantillas integradas no se pueden editar; duplícala".into());
    }
    let mut t = template;
    t.is_builtin = false;
    t.name = t.name.trim().to_owned();
    t.purpose = t.purpose.trim().to_owned();
    t.style = t.style.trim().to_owned();
    if t.name.is_empty() {
        return Err("la plantilla necesita un nombre".into());
    }
    for s in &mut t.structure {
        s.heading = s.heading.trim().to_owned();
        s.hint = s.hint.trim().to_owned();
    }
    repo::templates::upsert(&state.db, &t).await.map_err(e)?;
    Ok(t)
}

/// Borra una plantilla del usuario; las notas que la usaban vuelven a Auto.
#[tauri::command]
pub async fn delete_template(state: State<'_, AppState>, id: Id) -> Res<()> {
    if is_builtin(id) {
        return Err("las plantillas integradas no se pueden borrar".into());
    }
    repo::templates::delete(&state.db, id).await.map_err(e)
}

/// Fija la plantilla de la nota sin generar el resumen (menú `/` del editor);
/// `None` es Auto.
#[tauri::command]
pub async fn set_note_template(state: State<'_, AppState>, id: Id, template_id: Option<Id>) -> Res<()> {
    if repo::notes::get(&state.db, id).await.map_err(e)?.is_none() {
        return Err("la nota no existe".into());
    }
    match template_id {
        Some(tid) if !is_builtin(tid) => {
            if !repo::notes::set_user_template(&state.db, id, tid).await.map_err(e)? {
                return Err("la plantilla no existe".into());
            }
            Ok(())
        }
        _ => repo::notes::set_template(&state.db, id, template_id).await.map_err(e),
    }
}

/// La plantilla `id` lista para el enhance, o `None` si ya no existe (borrada).
async fn load_template(state: &AppState, id: Id, summary_language: &str) -> Res<Option<Template>> {
    if is_builtin(id) {
        return Ok(builtin_template(id, summary_language));
    }
    repo::templates::get(&state.db, id).await.map_err(e)
}

/// Genera las notas mejoradas de una nota a partir de sus notas crudas y su
/// transcripción, y las guarda en `enhanced_doc`.
///
/// `template`: sin él se usa la plantilla de la nota; «auto» o el id de una
/// plantilla la cambian (y se guarda en la nota) antes de generar.
#[tauri::command]
pub async fn enhance_note(app: AppHandle, state: State<'_, AppState>, id: Id, template: Option<String>) -> Res<EnhancedNotes> {
    // De uno en uno: dos enhances a la vez (misma nota o no) compiten por la
    // GPU y pisan el estado Enhancing/Ready del otro.
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = LOCK.lock().await;
    let set = |status: NoteStatus, message: Option<String>, notice: Option<&'static str>| {
        let _ = app.emit("note://status", NoteStatusEvent { note_id: id, status, message, notice });
    };
    let choice = match template.as_deref() {
        None => None,
        Some("auto") => Some(None),
        Some(t) => Some(Some(t.parse::<Id>().map_err(|_| format!("plantilla «{t}» no válida"))?)),
    };
    state.models.require(&["llm_main"])?;
    if let Some(template_id) = choice {
        repo::notes::set_template(&state.db, id, template_id).await.map_err(e)?;
    }
    repo::notes::set_status(&state.db, id, NoteStatus::Enhancing).await.map_err(e)?;
    set(NoteStatus::Enhancing, None, None);
    let result = async {
        let note = repo::notes::get(&state.db, id).await.map_err(e)?.ok_or("la nota no existe")?;
        let segments = repo::segments::list(&state.db, id).await.map_err(e)?;
        let model = llm(&state).await?;
        let summary_language = juanola_enhance::language::summary_language(&segments);
        let template = match note.template_id {
            Some(tid) => {
                let t = load_template(&state, tid, summary_language).await?;
                if t.is_none() {
                    tracing::warn!(template_id = %tid, "la plantilla de la nota ya no existe: se usa Auto");
                    repo::notes::set_template(&state.db, id, None).await.map_err(e)?;
                }
                t
            }
            None => None,
        };
        tracing::info!(summary_language, template = template.as_ref().map(|t| t.name.as_str()), "idioma del resumen");
        let input = EnhanceInput { note: &note, segments: &segments, template: template.as_ref(), summary_language, user_profile: "" };
        let (notes, report) = enhance(model.as_ref(), &input).await.map_err(e)?;
        tracing::info!(?report, "enhance");
        let value = serde_json::to_value(&notes).map_err(e)?;
        repo::notes::set_enhanced_doc(&state.db, id, Some(&value)).await.map_err(e)?;
        Ok::<_, String>((notes, report.template_fallback))
    }
    .await;
    match result {
        Ok((notes, fallback)) => {
            repo::notes::set_status(&state.db, id, NoteStatus::Ready).await.map_err(e)?;
            set(NoteStatus::Ready, None, fallback.then_some("template_fallback"));
            Ok(notes)
        }
        Err(msg) => {
            tracing::error!(note_id = %id, error = %msg, "enhance falló");
            repo::notes::set_status(&state.db, id, NoteStatus::Failed).await.map_err(e)?;
            set(NoteStatus::Failed, Some(msg.clone()), None);
            Err(msg)
        }
    }
}

// ------------------------------------------------------------------- otros

#[tauri::command]
pub async fn list_audio_devices() -> Res<Vec<juanola_audio_capture::AudioDevice>> {
    tauri::async_runtime::spawn_blocking(|| {
        use juanola_audio_capture::AudioCapture;
        juanola_audio_capture::CpalCapture::new().list_input_devices().unwrap_or_default()
    })
    .await
    .map_err(e)
}

// --------------------------------------------------- calendario y detección

/// Próximas reuniones de los próximos 30 días (para enseñar la siguiente
/// aunque no sea esta semana). Relee EventKit, unos milisegundos; vacío sin permiso.
#[tauri::command]
pub async fn coming_up() -> Res<Vec<CalendarEvent>> {
    let now = chrono::Utc::now();
    let events =
        tauri::async_runtime::spawn_blocking(move || detect::calendar_events(now, now + chrono::Duration::days(30))).await.map_err(e)?;
    Ok(juanola_calendar::coming_up(events, now))
}

#[tauri::command]
pub fn calendar_access() -> Access {
    detect::calendar_access()
}

/// Diagnóstico para el autotest: calendarios visibles y eventos del último y
/// el próximo mes (sin filtrar). Datos privados: solo con el autotest activo.
#[tauri::command]
pub async fn calendar_diagnostics() -> Res<Vec<String>> {
    if !autotest_enabled() {
        return Ok(Vec::new());
    }
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(target_os = "macos")]
        {
            let now = chrono::Utc::now();
            let mut out: Vec<String> =
                juanola_calendar::eventkit::calendars().into_iter().map(|(_, t, s)| format!("calendario: {t} ({s})")).collect();
            let events = juanola_calendar::eventkit::events(now - chrono::Duration::days(30), now + chrono::Duration::days(30));
            out.push(format!("eventos ±30 días (sin todo el día): {}", events.len()));
            out.extend(events.iter().rev().take(5).map(|e| format!("  {} · {} · {} asist.", e.start, e.title, e.attendees.len())));
            out
        }
        #[cfg(not(target_os = "macos"))]
        {
            Vec::new()
        }
    })
    .await
    .map_err(e)
}

/// Pide el permiso de calendario y espera la respuesta (hasta 2 min).
#[tauri::command]
pub async fn request_calendar_access(app: AppHandle) -> Res<Access> {
    let access = tauri::async_runtime::spawn_blocking(detect::request_calendar_access).await.map_err(e)?;
    // La Home vuelve a leer el calendario aunque el permiso se haya concedido desde otra vista.
    let _ = app.emit("calendar://changed", access);
    Ok(access)
}

/// Nota de un evento: la que ya tenga o una nueva con su título. Guarda una
/// copia del evento para que la nota conserve hora y asistentes.
async fn note_for_event(state: &AppState, event: &CalendarEvent) -> Res<Note> {
    // De uno en uno: dos clics seguidos (Coming up y el aviso, o un doble
    // clic) no deben crear dos notas para el mismo evento.
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = LOCK.lock().await;
    repo::events::upsert(&state.db, event).await.map_err(e)?;
    if let Some(n) = repo::notes::find_by_event(&state.db, &event.id).await.map_err(e)? {
        return Ok(n);
    }
    let ws = repo::workspaces::ensure_personal(&state.db).await.map_err(e)?;
    repo::notes::create_for_event(&state.db, ws, &event.title, &event.id).await.map_err(e)
}

#[tauri::command]
pub async fn open_event_note(state: State<'_, AppState>, event: CalendarEvent) -> Res<Note> {
    // El evento llega del webview: no envenenar la caché con ids arbitrarios.
    if event.id.trim().is_empty() || event.id.len() > 256 || event.provider.trim().is_empty() || event.provider.len() > 64 {
        return Err("evento no válido".into());
    }
    note_for_event(&state, &event).await
}

/// Evento asociado a una nota, si lo tiene.
#[tauri::command]
pub async fn note_event(state: State<'_, AppState>, note_id: Id) -> Res<Option<CalendarEvent>> {
    let Some(note) = repo::notes::get(&state.db, note_id).await.map_err(e)? else { return Ok(None) };
    match note.calendar_event_id {
        Some(id) => repo::events::get(&state.db, &id).await.map_err(e),
        None => Ok(None),
    }
}

/// Glosario del usuario para la transcripción (texto tal cual lo escribió).
#[tauri::command]
pub async fn stt_glossary(state: State<'_, AppState>) -> Res<String> {
    Ok(repo::settings::get(&state.db, crate::recording::KEY_GLOSSARY).await.map_err(e)?.unwrap_or_default())
}

#[tauri::command]
pub async fn set_stt_glossary(state: State<'_, AppState>, glossary: String) -> Res<()> {
    repo::settings::set(&state.db, crate::recording::KEY_GLOSSARY, &glossary).await.map_err(e)
}

#[tauri::command]
pub fn detection_settings(state: State<'_, AppState>) -> DetectionSettings {
    state.detection.settings()
}

#[tauri::command]
pub async fn set_detection_settings(app: AppHandle, state: State<'_, AppState>, settings: DetectionSettings) -> Res<()> {
    state.detection.set_settings(settings);
    detect::close_disabled(&app, settings);
    repo::settings::set(&state.db, detect::KEY_MEETINGS, &settings.meetings).await.map_err(e)?;
    repo::settings::set(&state.db, detect::KEY_REMINDERS, &settings.calendar_reminders).await.map_err(e)?;
    repo::settings::set(&state.db, detect::KEY_AUTO_STOP, &settings.auto_stop).await.map_err(e)
}

/// Lo que muestra la ventana de aviso al abrirse.
#[tauri::command]
pub fn notification_payload(state: State<'_, AppState>) -> Option<Notification> {
    state.detection.current()
}

/// «Take Notes»: abre (o crea) la nota, trae la ventana principal y le pide
/// que empiece a grabar. En un aviso de calendario abre también la videollamada.
#[tauri::command]
pub async fn notification_take_notes(app: AppHandle, state: State<'_, AppState>) -> Res<()> {
    // `close` se queda el aviso: un doble clic no crea dos notas.
    let Some(n) = detect::close(&app) else { return Ok(()) };
    let note = match &n.event {
        Some(ev) => note_for_event(&state, ev).await?,
        None => {
            let ws = repo::workspaces::ensure_personal(&state.db).await.map_err(e)?;
            repo::notes::create(&state.db, ws, "").await.map_err(e)?
        }
    };
    if n.kind == NotificationKind::Calendar
        && let Some(url) = n.event.as_ref().and_then(|ev| ev.conference_url.clone()).filter(|u| juanola_calendar::is_conference_url(u))
    {
        use tauri_plugin_opener::OpenerExt;
        if let Err(err) = app.opener().open_url(url, None::<&str>) {
            tracing::warn!(error = %err, "no se pudo abrir la videollamada");
        }
    }
    state.detection.set_origin(match &n.app {
        Some(a) => detect::Origin::App(a.bundle_id.clone()),
        None => detect::Origin::Pending,
    });
    detect::focus_main(&app);
    tracing::info!(note_id = %note.id, "take notes");
    app.emit_to("main", "detect://take-notes", TakeNotes { note_id: note.id }).map_err(e)
}

#[tauri::command]
pub fn notification_dismiss(app: AppHandle) {
    detect::close(&app);
}

/// Muestra un aviso de prueba («Probar aviso» en Ajustes y autotest): `kind`
/// es `"adhoc"` o `"calendar"`.
#[tauri::command]
pub fn simulate_detection(app: AppHandle, kind: String) -> Res<()> {
    let now = chrono::Utc::now();
    let n = if kind == "calendar" {
        let event = CalendarEvent {
            id: format!("test:{}", now.timestamp()),
            provider: "test".into(),
            calendar_id: "test".into(),
            title: "Reunión de prueba".into(),
            start: now + chrono::Duration::seconds(50),
            end: now + chrono::Duration::minutes(30),
            attendees: vec![],
            conference_url: None,
            recurring_event_id: None,
            description: None,
        };
        detect::calendar_notification(&event, now)
    } else {
        let detected = juanola_detect::classify("us.zoom.xos").ok_or("sin app")?;
        detect::adhoc_notification(detected, None)
    };
    detect::show(&app, n);
    Ok(())
}

/// Abre el panel de privacidad de Ajustes del Sistema que corresponde a `kind`
/// (`"mic_settings"` o `"audio_capture_settings"`).
#[tauri::command]
pub async fn open_privacy_settings(kind: String) -> Res<()> {
    let pane = match kind.as_str() {
        "mic_settings" => "Privacy_Microphone",
        "audio_capture_settings" => "Privacy_ScreenCapture",
        "calendar_settings" => "Privacy_Calendars",
        other => return Err(format!("ajuste desconocido: {other}")),
    };
    let out = tauri::async_runtime::spawn_blocking(move || {
        std::process::Command::new("open").arg(format!("x-apple.systempreferences:com.apple.preference.security?{pane}")).status()
    })
    .await
    .map_err(e)?
    .map_err(e)?;
    if out.success() { Ok(()) } else { Err(format!("open terminó con {out}")) }
}

/// Sin caracteres de control ni separadores Unicode: no se pueden falsificar
/// líneas de log ni meter secuencias ANSI.
fn log_clean(s: &str) -> String {
    s.chars().map(|c| if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') { ' ' } else { c }).collect()
}

/// Error de pintado del frontend (lo captura su ErrorBoundary): al log.
#[tauri::command]
pub fn frontend_error(message: String) {
    tracing::error!(target: "frontend", "{}", log_clean(&message));
}

/// Estado del permiso de micrófono para la pantalla de Ajustes. (El de audio
/// del sistema no tiene API de consulta: solo se sabe al grabar.)
#[tauri::command]
pub fn mic_permission_status() -> juanola_audio_capture::Permission {
    juanola_audio_capture::mic_permission()
}

/// Abre la carpeta de datos de Juanola en Finder.
#[tauri::command]
pub async fn reveal_data_dir(state: State<'_, AppState>) -> Res<()> {
    let dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || std::process::Command::new("open").arg(dir).status()).await.map_err(e)?.map_err(e)?;
    Ok(())
}

// --------------------------------------------------------- autotest (desarrollo)
//
// Con `JUANOLA_AUTOTEST=1` el frontend recorre solo el flujo completo pulsando
// los botones reales (ver `src/dev/autotest.ts`) y deja el resultado en el log.
// Sirve para probar la .app con permisos reales sin tocar la ventana.

fn autotest_enabled() -> bool {
    // Los comandos `autotest_*` leen el entorno y lanzan `say`/`afplay`: en
    // release no se activan aunque alguien ponga la variable.
    cfg!(debug_assertions) && std::env::var_os("JUANOLA_AUTOTEST").is_some_and(|v| v != "0")
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

/// Variable de entorno para el autotest (`None` fuera de él).
#[tauri::command]
pub fn autotest_env(key: String) -> Option<String> {
    (autotest_enabled() && key.starts_with("JUANOLA_")).then(|| std::env::var(key).ok()).flatten()
}

#[tauri::command]
pub fn autotest_log(line: String) {
    if autotest_enabled() {
        tracing::info!(target: "autotest", "{}", log_clean(&line));
    }
}

/// Reproduce con `afplay` el WAV de `JUANOLA_AUTOTEST_AUDIO` por la salida del
/// sistema y vuelve al terminar (modo `context`).
#[tauri::command]
pub async fn autotest_play() -> Res<()> {
    let path = std::env::var_os("JUANOLA_AUTOTEST_AUDIO").filter(|_| autotest_enabled()).ok_or("falta JUANOLA_AUTOTEST_AUDIO")?;
    tauri::async_runtime::spawn_blocking(move || std::process::Command::new("afplay").arg(path).status().map_err(e).map(|_| ()))
        .await
        .map_err(e)?
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
        cmd.arg("--").arg(text).status().map_err(e).map(|_| ())
    })
    .await
    .map_err(e)?
}

// ---------------------------------------------------------------- indicador

/// Sesión que muestran la barra de menús y el indicador flotante.
#[tauri::command]
pub fn indicator_state(state: State<'_, AppState>) -> Option<crate::indicator::Snapshot> {
    state.indicator.snapshot()
}

/// Indicador: trae la ventana principal con la nota que se graba.
#[tauri::command]
pub fn indicator_open(app: AppHandle) {
    crate::indicator::open_note(&app);
}

/// Indicador: pausar (`true`) o reanudar (`false`).
#[tauri::command]
pub fn indicator_pause(app: AppHandle, pause: bool) {
    crate::indicator::pause(&app, pause);
}

/// Indicador: pide a la ventana principal que pare (y resuma).
#[tauri::command]
pub fn indicator_stop(app: AppHandle) {
    crate::indicator::request_stop(&app);
}

/// Textos del menú de la barra en el idioma de la interfaz.
#[tauri::command]
pub fn set_tray_labels(app: AppHandle, labels: crate::indicator::Labels) {
    crate::indicator::set_labels(&app, labels);
}

// ------------------------------------------------------------------ exportar

fn enhanced_of(note: &Note) -> Option<EnhancedNotes> {
    note.enhanced_doc.clone().and_then(|v| serde_json::from_value(v).ok())
}

/// El resumen de la nota en Markdown, para copiarlo (vacío si no hay).
#[tauri::command]
pub async fn note_summary_markdown(state: State<'_, AppState>, id: Id) -> Res<String> {
    let note = repo::notes::get(&state.db, id).await.map_err(e)?.ok_or("la nota no existe")?;
    Ok(enhanced_of(&note).map(|n| crate::export::summary_markdown(&n)).unwrap_or_default())
}

/// Exporta la nota: pide dónde guardarla (diálogo del sistema) y la escribe.
/// La ruta la elige el usuario en el diálogo, nunca el webview. Devuelve la
/// ruta, o `None` si se canceló.
#[tauri::command]
pub async fn export_note(
    app: AppHandle,
    state: State<'_, AppState>,
    id: Id,
    format: crate::export::Format,
    labels: crate::export::Labels,
) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let note = repo::notes::get(&state.db, id).await.map_err(e)?.ok_or("la nota no existe")?;
    let segments = repo::segments::list(&state.db, id).await.map_err(e)?;
    let enhanced = enhanced_of(&note);
    let doc = crate::export::document(&note, enhanced.as_ref(), &segments, &labels, format);
    let (name, ext) = (crate::export::file_name(&note, &labels, format), format.extension());
    let filter = if format == crate::export::Format::Html { "HTML" } else { "Markdown" };
    // El autotest no puede contestar al diálogo: escribe en esta carpeta.
    let mut path =
        if let Some(dir) = std::env::var_os("JUANOLA_AUTOTEST_EXPORT_DIR").filter(|_| std::env::var_os("JUANOLA_AUTOTEST").is_some()) {
            std::path::PathBuf::from(dir).join(name)
        } else {
            let dialog = app.dialog().file().set_file_name(name).add_filter(filter, &[ext]);
            let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file()).await.map_err(e)?;
            let Some(path) = picked else { return Ok(None) };
            path.into_path().map_err(e)?
        };
    if path.extension().and_then(|x| x.to_str()) != Some(ext) {
        path.set_extension(ext);
    }
    std::fs::write(&path, doc).map_err(e)?;
    tracing::info!(path = %path.display(), ?format, "nota exportada");
    Ok(Some(path.display().to_string()))
}
