//! Sesión de grabación: captura (mic + sistema) → `juanola-stt-worker` →
//! eventos al frontend y segmentos finales a SQLite.
//!
//! Corre en un hilo propio que es dueño de la captura y del proceso hijo de
//! STT. Eventos que emite:
//! - `recording://state`  `{ note_id, state: "starting"|"recording"|"paused"|"finishing"|"stopped"|"error", message? }`
//! - `recording://levels` `{ mic, system }` (RMS, ~20 Hz)
//! - `transcript://segment` `{ note_id, channel, id, start, end, text, is_final }`

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use juanola_audio_capture::{rms, AudioCapture, CaptureConfig, CpalCapture};
use juanola_core_types::{Channel, Id, Segment};
use juanola_db::{repo, Db};
use juanola_stt::context::{parse_glossary, AsrContext};
use juanola_stt::worker::{SttWorker, WorkerConfig, WorkerEvent};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

enum Ctl {
    Pause,
    Resume,
    Stop,
}

pub struct Active {
    pub note_id: Id,
    ctl: mpsc::Sender<Ctl>,
    handle: JoinHandle<Result<usize, String>>,
}

#[derive(Serialize, Clone)]
struct StateEvent<'a> {
    note_id: Id,
    state: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    /// Ajuste del sistema que arregla el problema: `"mic_settings"` o
    /// `"audio_capture_settings"` (el frontend muestra un botón).
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<&'static str>,
}

/// Fin con error de una sesión: el mensaje y, si lo hay, el ajuste que lo arregla.
struct Failure {
    message: String,
    action: Option<&'static str>,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self { message, action: None }
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
}

const MIC_DENIED: &str = "Juanola no tiene permiso para usar el micrófono. Actívalo en Ajustes del Sistema → Privacidad y seguridad → Micrófono y vuelve a pulsar Start.";
const MIC_ASK: &str = "macOS pide permiso para usar el micrófono: acéptalo para empezar a grabar.";
const MIC_UNANSWERED: &str = "No se respondió al permiso de micrófono. Vuelve a pulsar Start o actívalo en Ajustes del Sistema.";
const MIC_SILENT: &str = "El micrófono solo da silencio. Si no lo tienes silenciado, revisa el permiso de micrófono de Juanola.";
const SYSTEM_SILENT: &str = "El audio del sistema llega en silencio aunque algo suena. Revisa que Juanola tenga permiso en Ajustes del Sistema → Privacidad y seguridad → Grabación de pantalla y audio del sistema.";
/// Segundos de audio (y de reloj) solo con ceros antes de avisar.
const SILENT_MIC_SECS: f32 = 4.0;
const SILENT_SYSTEM_SECS: f32 = 6.0;

#[derive(Serialize, Clone)]
struct Levels {
    mic: f32,
    system: f32,
}

#[derive(Serialize, Clone)]
struct SegmentEvent {
    note_id: Id,
    channel: Channel,
    /// Único por nota y canal: `"<canal>-<sesión>-<id del worker>"`.
    id: String,
    start: f64,
    end: f64,
    text: String,
    is_final: bool,
}

/// Rutas de los modelos y del worker.
#[derive(Clone)]
pub struct Paths {
    pub stt_dir: PathBuf,
    pub vad_dir: PathBuf,
}

impl Paths {

    /// El worker es un sidecar de Tauri (`bundle.externalBin`): va junto al
    /// ejecutable, en `Contents/MacOS` dentro del .app y en `target/<perfil>`
    /// con `tauri dev`. Lo genera `apps/desktop/scripts/build-sidecar.sh`.
    /// Último recurso, el de `target/release` del workspace.
    pub fn worker(&self) -> PathBuf {
        if let Some(p) = std::env::var_os("JUANOLA_STT_WORKER") {
            return p.into();
        }
        if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)) {
            let side = dir.join("juanola-stt-worker");
            if side.exists() {
                return side;
            }
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/release/juanola-stt-worker")
    }
}

fn emit_state(app: &AppHandle, note_id: Id, state: &str, message: Option<String>) {
    emit_notice(app, note_id, state, message, None);
}

fn emit_notice(app: &AppHandle, note_id: Id, state: &str, message: Option<String>, action: Option<&'static str>) {
    let _ = app.emit("recording://state", StateEvent { note_id, state, message, action });
}

impl Active {
    pub fn start(app: AppHandle, db: Db, paths: Paths, note_id: Id, language: Option<String>) -> Result<Self, String> {
        let worker_path = paths.worker();
        if !worker_path.exists() {
            return Err(format!(
                "no encuentro juanola-stt-worker en {}; genéralo con `apps/desktop/scripts/build-sidecar.sh`",
                worker_path.display()
            ));
        }
        let (ctl, ctl_rx) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("recording".into())
            .spawn(move || {
                let res = run(&app, &db, &paths, worker_path, note_id, language, ctl_rx);
                match &res {
                    Ok(_) => emit_state(&app, note_id, "stopped", None),
                    Err(f) => {
                        tracing::error!(%note_id, error = %f.message, "la grabación terminó con error");
                        emit_notice(&app, note_id, "error", Some(f.message.clone()), f.action);
                    }
                }
                res.map_err(|f| f.message)
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { note_id, ctl, handle })
    }

    pub fn pause(&self) {
        let _ = self.ctl.send(Ctl::Pause);
    }

    pub fn resume(&self) {
        let _ = self.ctl.send(Ctl::Resume);
    }

    /// Para y espera a que se transcriba lo pendiente. Devuelve cuántos
    /// segmentos finales se guardaron.
    pub fn stop(self) -> Result<usize, String> {
        let _ = self.ctl.send(Ctl::Stop);
        self.handle.join().map_err(|_| "el hilo de grabación se cayó".to_string())?
    }
}

/// Convierte un evento del worker en segmento (si es final) y lo publica.
struct Sink<'a> {
    app: &'a AppHandle,
    db: &'a Db,
    note_id: Id,
    offset: f64,
    session: u64,
    saved: usize,
    partials: usize,
    finals: usize,
}

impl Sink<'_> {
    fn handle(&mut self, ev: WorkerEvent) {
        match ev {
            WorkerEvent::Segment { channel, id, start, end, text, is_final, .. } => {
                if is_final {
                    self.finals += 1;
                    tracing::info!(?channel, start, end, text = %text, "segmento final");
                } else {
                    self.partials += 1;
                }
                let (start, end) = (start + self.offset, end + self.offset);
                let label = match channel {
                    Channel::Mic => "mic",
                    Channel::System => "system",
                };
                let _ = self.app.emit(
                    "transcript://segment",
                    SegmentEvent {
                        note_id: self.note_id,
                        channel,
                        id: format!("{label}-{}-{id}", self.session),
                        start,
                        end,
                        text: text.clone(),
                        is_final,
                    },
                );
                if is_final && !text.trim().is_empty() {
                    let seg = Segment {
                        id: Id::new_v4(),
                        note_id: self.note_id,
                        channel,
                        speaker_id: None,
                        start,
                        end,
                        text,
                        confidence: None,
                        is_final: true,
                    };
                    match tauri::async_runtime::block_on(repo::segments::insert(self.db, &seg)) {
                        Ok(()) => self.saved += 1,
                        Err(e) => tracing::error!("no se pudo guardar un segmento: {e}"),
                    }
                }
            }
            WorkerEvent::Error { message } => {
                tracing::warn!("worker STT: {message}");
                emit_state(self.app, self.note_id, "warning", Some(message));
            }
            WorkerEvent::Ready { .. } => {}
        }
    }
}

/// Clave del glosario del usuario en `settings`: texto libre, un término por
/// línea o separados por comas.
pub const KEY_GLOSSARY: &str = "stt.glossary";

/// Lo que se sabe de la reunión antes de oírla, para el prompt del STT: título,
/// evento de calendario, notas ya escritas y glosario. Si algo falla, se graba
/// sin contexto.
fn asr_context(db: &Db, note_id: Id) -> Option<String> {
    let ctx = tauri::async_runtime::block_on(async {
        let note = repo::notes::get(db, note_id).await.ok().flatten()?;
        let event = match &note.calendar_event_id {
            Some(id) => repo::events::get(db, id).await.ok().flatten(),
            None => None,
        };
        let glossary: String = repo::settings::get(db, KEY_GLOSSARY).await.ok().flatten().unwrap_or_default();
        let notes = juanola_enhance::pipelines::enhance::user_blocks(&note.raw_doc)
            .into_iter()
            .map(|b| b.text)
            .collect::<Vec<_>>()
            .join(". ");
        Some(AsrContext {
            title: note.title,
            attendees: event
                .as_ref()
                .map(|e| e.attendees.iter().filter(|a| !a.declined).filter_map(|a| a.name.clone()).collect())
                .unwrap_or_default(),
            description: event.and_then(|e| e.description).unwrap_or_default(),
            notes,
            glossary: parse_glossary(&glossary),
        })
    })?;
    let text = ctx.render();
    tracing::info!(%note_id, chars = text.as_ref().map_or(0, |t| t.chars().count()), "contexto del STT");
    text
}

fn run(
    app: &AppHandle,
    db: &Db,
    paths: &Paths,
    program: PathBuf,
    note_id: Id,
    language: Option<String>,
    ctl: mpsc::Receiver<Ctl>,
) -> Result<usize, Failure> {
    emit_state(app, note_id, "starting", None);
    // macOS no da error si el micrófono está denegado: entrega ceros. Si aún no
    // se ha decidido, se pide ya y se espera la respuesta: con el diálogo en
    // pantalla, abrir el micrófono se quedaría bloqueado.
    // Solo para el autotest sin nadie delante: sin micrófono (no hay quien
    // acepte su permiso tras cada recompilación con firma ad hoc).
    let capture_mic = std::env::var_os("JUANOLA_AUTOTEST_NO_MIC").is_none();
    let mut mic_permission = if capture_mic {
        juanola_audio_capture::mic_permission()
    } else {
        tracing::warn!("JUANOLA_AUTOTEST_NO_MIC: se graba sin micrófono");
        juanola_audio_capture::Permission::Unknown
    };
    if mic_permission == juanola_audio_capture::Permission::NotDetermined {
        emit_notice(app, note_id, "warning", Some(MIC_ASK.into()), None);
        mic_permission = juanola_audio_capture::request_mic_permission(Duration::from_secs(120));
        emit_notice(app, note_id, "warning", None, None);
    }
    tracing::info!(?mic_permission, "permiso de micrófono");
    match mic_permission {
        juanola_audio_capture::Permission::Denied => {
            return Err(Failure { message: MIC_DENIED.into(), action: Some("mic_settings") });
        }
        juanola_audio_capture::Permission::NotDetermined => {
            return Err(Failure { message: MIC_UNANSWERED.into(), action: Some("mic_settings") });
        }
        _ => {}
    }
    let mut worker = SttWorker::spawn(&WorkerConfig {
        program,
        model_dir: paths.stt_dir.clone(),
        vad_dir: paths.vad_dir.clone(),
        language,
        context: asr_context(db, note_id),
    })
    .map_err(|e| e.to_string())?;
    match worker.events().recv_timeout(Duration::from_secs(120)) {
        Ok(WorkerEvent::Ready { load_ms }) => tracing::info!(load_ms, "STT listo"),
        Ok(WorkerEvent::Error { message }) => return Err(format!("el STT no arrancó: {message}").into()),
        Ok(other) => return Err(format!("el STT no arrancó: {other:?}").into()),
        Err(_) => return Err("el STT no arrancó a tiempo".into()),
    }

    // Retomar una nota ya grabada: los tiempos siguen tras lo último guardado.
    let offset = tauri::async_runtime::block_on(repo::segments::last_end(db, note_id))
        .map(|e| if e > 0.0 { e + 1.0 } else { 0.0 })
        .map_err(|e| e.to_string())?;
    let session = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    tracing::info!(%note_id, offset, "grabación iniciada");
    let mut sink = Sink { app, db, note_id, offset, session, saved: 0, partials: 0, finals: 0 };

    let mut capture = CpalCapture::new();
    let mut frames = capture
        .start(CaptureConfig { mic_device: None, capture_mic, capture_system_audio: true, echo_cancellation: false })
        .map_err(|e| match e {
            juanola_audio_capture::CaptureError::PermissionDenied(_) => {
                Failure { message: MIC_DENIED.into(), action: Some("mic_settings") }
            }
            other => Failure::from(other.to_string()),
        })?;
    emit_state(app, note_id, "recording", None);

    let (mut mic_acc, mut sys_acc) = (Vec::<f32>::new(), Vec::<f32>::new());
    // Diagnóstico: muestras y pico por canal (se registra al parar).
    let (mut mic_n, mut sys_n, mut mic_peak, mut sys_peak) = (0usize, 0usize, 0f32, 0f32);
    let started = Instant::now();
    let mut last_levels = Instant::now();
    // Avisos de silencio sospechoso (permiso denegado): una vez por sesión.
    // El del sistema exige además reloj desde el primer callback, porque el
    // relleno de huecos puede traer de golpe muchos segundos de ceros justo
    // antes del audio real.
    let (mut mic_warned, mut sys_warned) = (false, false);
    let mut sys_first: Option<Instant> = None;
    loop {
        match ctl.try_recv() {
            Ok(Ctl::Stop) | Err(mpsc::TryRecvError::Disconnected) => break,
            Ok(Ctl::Pause) => {
                let _ = capture.pause();
                emit_state(app, note_id, "paused", None);
            }
            Ok(Ctl::Resume) => {
                let _ = capture.resume();
                emit_state(app, note_id, "recording", None);
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        // Con el micro siempre llegan frames (~10 ms); en pausa, no: dormir un poco.
        match frames.try_recv() {
            Ok(f) => {
                let peak = f.samples.iter().fold(0f32, |m, s| m.max(s.abs()));
                match f.channel {
                    Channel::Mic => {
                        mic_acc.extend_from_slice(&f.samples);
                        mic_n += f.samples.len();
                        mic_peak = mic_peak.max(peak);
                    }
                    Channel::System => {
                        sys_first.get_or_insert_with(Instant::now);
                        sys_acc.extend_from_slice(&f.samples);
                        sys_n += f.samples.len();
                        sys_peak = sys_peak.max(peak);
                    }
                }
                if let Err(e) = worker.send(f.channel, &f.samples) {
                    let status = worker.exit_status();
                    return Err(format!("el STT dejó de aceptar audio ({e}); salida del worker: {status:?}").into());
                }
                if !mic_warned && mic_peak == 0.0 && mic_n as f32 / 16_000.0 >= SILENT_MIC_SECS {
                    mic_warned = true;
                    tracing::warn!("el micrófono solo da ceros");
                    emit_notice(app, note_id, "warning", Some(MIC_SILENT.into()), Some("mic_settings"));
                }
                if !sys_warned
                    && sys_peak == 0.0
                    && sys_n as f32 / 16_000.0 >= SILENT_SYSTEM_SECS
                    && sys_first.is_some_and(|t| t.elapsed().as_secs_f32() >= SILENT_SYSTEM_SECS)
                {
                    // El tap no entrega nada mientras no suena nada; si entrega
                    // ceros durante segundos, lo normal es que falte el permiso.
                    sys_warned = true;
                    tracing::warn!("el audio del sistema solo da ceros");
                    emit_notice(app, note_id, "warning", Some(SYSTEM_SILENT.into()), Some("audio_capture_settings"));
                }
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => tracing::warn!("{n} frames de audio perdidos"),
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(5)),
        }
        if last_levels.elapsed() >= Duration::from_millis(50) {
            let _ = app.emit("recording://levels", Levels { mic: rms(&mic_acc), system: rms(&sys_acc) });
            mic_acc.clear();
            sys_acc.clear();
            last_levels = Instant::now();
        }
        for ev in worker.events().try_iter() {
            sink.handle(ev);
        }
    }

    tracing::info!(
        secs = started.elapsed().as_secs_f32(),
        mic_audio_s = mic_n as f32 / 16_000.0,
        mic_peak,
        system_audio_s = sys_n as f32 / 16_000.0,
        system_peak = sys_peak,
        "grabación parada"
    );
    emit_state(app, note_id, "finishing", None);
    let _ = capture.stop();
    // Vaciar lo que quede en el broadcast antes de cerrar el worker.
    loop {
        match frames.try_recv() {
            Ok(f) => {
                let _ = worker.send(f.channel, &f.samples);
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
    }
    worker.finish();
    while let Ok(ev) = worker.events().recv_timeout(Duration::from_secs(30)) {
        sink.handle(ev);
    }
    let _ = worker.wait();
    tracing::info!(saved = sink.saved, finals = sink.finals, partials = sink.partials, "transcripción terminada");
    Ok(sink.saved)
}
