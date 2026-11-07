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
}

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
    pub data_dir: PathBuf,
}

impl Paths {
    pub fn models(&self) -> PathBuf {
        self.data_dir.join("models")
    }

    /// El worker va junto al ejecutable dentro del .app; en desarrollo se usa el
    /// de `target/release` del workspace (compílalo con
    /// `cargo build --release -p juanola-stt --features mlx --bin juanola-stt-worker`).
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
    let _ = app.emit("recording://state", StateEvent { note_id, state, message });
}

impl Active {
    pub fn start(app: AppHandle, db: Db, paths: Paths, note_id: Id, language: Option<String>) -> Result<Self, String> {
        let worker_path = paths.worker();
        if !worker_path.exists() {
            return Err(format!(
                "no encuentro juanola-stt-worker en {}; compílalo con `cargo build --release -p juanola-stt --features mlx --bin juanola-stt-worker`",
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
                    Err(e) => {
                        tracing::error!(%note_id, error = %e, "la grabación terminó con error");
                        emit_state(&app, note_id, "error", Some(e.clone()));
                    }
                }
                res
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

fn run(
    app: &AppHandle,
    db: &Db,
    paths: &Paths,
    program: PathBuf,
    note_id: Id,
    language: Option<String>,
    ctl: mpsc::Receiver<Ctl>,
) -> Result<usize, String> {
    emit_state(app, note_id, "starting", None);
    let models = paths.models();
    let mut worker = SttWorker::spawn(&WorkerConfig {
        program,
        model_dir: models.join("stt_streaming/qwen3-asr-0.6b-mlx"),
        vad_dir: models.join("vad/silero-vad-mlx"),
        language,
    })
    .map_err(|e| e.to_string())?;
    match worker.events().recv_timeout(Duration::from_secs(120)) {
        Ok(WorkerEvent::Ready { load_ms }) => tracing::info!(load_ms, "STT listo"),
        Ok(WorkerEvent::Error { message }) => return Err(format!("el STT no arrancó: {message}")),
        Ok(other) => return Err(format!("el STT no arrancó: {other:?}")),
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
        .start(CaptureConfig { mic_device: None, capture_system_audio: true, echo_cancellation: false })
        .map_err(|e| e.to_string())?;
    emit_state(app, note_id, "recording", None);

    let (mut mic_acc, mut sys_acc) = (Vec::<f32>::new(), Vec::<f32>::new());
    // Diagnóstico: muestras y pico por canal (se registra al parar).
    let (mut mic_n, mut sys_n, mut mic_peak, mut sys_peak) = (0usize, 0usize, 0f32, 0f32);
    let started = Instant::now();
    let mut last_levels = Instant::now();
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
                        sys_acc.extend_from_slice(&f.samples);
                        sys_n += f.samples.len();
                        sys_peak = sys_peak.max(peak);
                    }
                }
                if let Err(e) = worker.send(f.channel, &f.samples) {
                    let status = worker.exit_status();
                    return Err(format!("el STT dejó de aceptar audio ({e}); salida del worker: {status:?}"));
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
    if mic_n > 0 && mic_peak == 0.0 {
        emit_state(app, note_id, "warning", Some("El micrófono solo ha dado silencio: revisa el permiso de micrófono de Juanola.".into()));
    }
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
