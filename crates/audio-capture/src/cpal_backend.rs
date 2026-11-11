//! Backend de captura sobre `cpal` para micrófono y audio del sistema.
//!
//! - Micrófono: dispositivo de entrada por defecto o el elegido por id.
//! - Sistema: `build_input_stream` sobre el dispositivo de *salida* por defecto.
//!   En macOS 14.6+ cpal crea un Core Audio process tap (todas las apps, sin
//!   silenciar los altavoces) y un aggregate device privado; en Windows usa
//!   WASAPI loopback.
//!
//! Hilos:
//! - `audio-owner`: crea y posee los `cpal::Stream` (no son `Send` en todas las
//!   plataformas), atiende Pause/Resume/Stop y reconstruye un stream cuando el
//!   dispositivo cambia (p. ej. unos auriculares Bluetooth pasan a perfil manos
//!   libres al abrir su micrófono y cambian la frecuencia de salida).
//! - `audio-dsp-{mic,system}`: reciben los bloques ya en mono desde el callback,
//!   los pasan a 16 kHz, rellenan con silencio los huecos de una reconstrucción
//!   (para que los dos canales sigan alineados en el tiempo) y publican `AudioFrame`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, InterfaceType, SampleFormat, SizedSample};
use juanola_core_types::Channel;
use tokio::sync::broadcast;

use crate::resample::MonoResampler;
use crate::{AudioCapture, AudioDevice, AudioFrame, CaptureConfig, CaptureError, TARGET_SAMPLE_RATE};

/// Bloques en vuelo entre el callback de audio y el hilo DSP (~10 ms cada uno).
const CALLBACK_QUEUE: usize = 512;
/// Frames en el broadcast hacia los consumidores (STT, VU, WAV).
const BROADCAST_CAPACITY: usize = 1024;
/// Espera antes de reconstruir, para agrupar las ráfagas de avisos de Core Audio.
const REBUILD_SETTLE: Duration = Duration::from_millis(300);
const REBUILD_RETRY: Duration = Duration::from_secs(1);
/// Huecos menores que esto no se rellenan (jitter normal de los callbacks).
const GAP_TOLERANCE_SECS: f64 = 0.05;
/// Si un stream en marcha no entrega ningún callback en este tiempo, se
/// reconstruye. Pasa con el tap cuando la salida Bluetooth cambia de perfil y
/// Core Audio no lo notifica.
const STALL_TIMEOUT: Duration = Duration::from_secs(2);
const WATCHDOG_TICK: Duration = Duration::from_millis(500);
/// Cada cuánto mira el DSP si se ha pedido el cierre.
const DSP_POLL: Duration = Duration::from_millis(100);
/// Tope para abrir un stream (Core Audio a veces no vuelve nunca). El mic es
/// más generoso: con Bluetooth tarda ~3 s en cambiar de perfil y, si falla, la
/// grabación no puede seguir; el del sistema se reintenta en segundo plano.
const OPEN_TIMEOUT_MIC: Duration = Duration::from_secs(15);
const OPEN_TIMEOUT_SYSTEM: Duration = Duration::from_secs(6);

enum Command {
    Pause,
    Resume,
    Stop,
    Rebuild(Channel),
}

struct Running {
    commands: mpsc::Sender<Command>,
    /// Origen de los `AudioFrame::timestamp` de esta sesión.
    t0: Instant,
    owner: JoinHandle<()>,
}

#[derive(Default)]
pub struct CpalCapture {
    running: Option<Running>,
}

impl CpalCapture {
    pub fn new() -> Self {
        Self::default()
    }

    /// Instante que corresponde a `timestamp = 0` en los frames de la sesión
    /// activa. Es anterior a la vuelta de `start()`: abrir dispositivos puede
    /// tardar segundos (p. ej. auriculares Bluetooth cambiando de perfil).
    pub fn session_start(&self) -> Option<Instant> {
        self.running.as_ref().map(|r| r.t0)
    }

    fn send(&self, cmd: Command) -> Result<(), CaptureError> {
        let r = self.running.as_ref().ok_or_else(|| CaptureError::Backend("no hay captura activa".into()))?;
        r.commands.send(cmd).map_err(|_| CaptureError::Backend("el hilo de audio terminó".into()))
    }
}

impl AudioCapture for CpalCapture {
    fn list_input_devices(&self) -> Result<Vec<AudioDevice>, CaptureError> {
        let host = cpal::default_host();
        let default_id = host.default_input_device().and_then(|d| d.id().ok());
        let devices = host.input_devices().map_err(backend)?;
        Ok(devices
            .filter_map(|d| {
                let id = d.id().ok()?;
                let desc = d.description().ok()?;
                Some(AudioDevice {
                    is_default: Some(&id) == default_id.as_ref(),
                    id: id.to_string(),
                    name: desc.name().to_string(),
                    is_bluetooth: desc.interface_type() == InterfaceType::Bluetooth,
                })
            })
            .collect())
    }

    fn start(&mut self, cfg: CaptureConfig) -> Result<broadcast::Receiver<AudioFrame>, CaptureError> {
        if self.running.is_some() {
            return Err(CaptureError::Backend("la captura ya está en marcha".into()));
        }
        let (frames_tx, frames_rx) = broadcast::channel(BROADCAST_CAPACITY);
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();

        let owner_cmd_tx = cmd_tx.clone();
        let owner = std::thread::Builder::new()
            .name("audio-owner".into())
            .spawn(move || owner_thread(cfg, frames_tx, owner_cmd_tx, cmd_rx, ready_tx))
            .map_err(backend)?;

        match ready_rx.recv() {
            Ok(Ok(t0)) => {
                self.running = Some(Running { commands: cmd_tx, t0, owner });
                Ok(frames_rx)
            }
            Ok(Err(e)) => {
                let _ = owner.join();
                Err(e)
            }
            Err(_) => Err(CaptureError::Backend("el hilo de audio murió al arrancar".into())),
        }
    }

    fn pause(&mut self) -> Result<(), CaptureError> {
        self.send(Command::Pause)
    }

    fn resume(&mut self) -> Result<(), CaptureError> {
        self.send(Command::Resume)
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        if let Some(r) = self.running.take() {
            let _ = r.commands.send(Command::Stop);
            let _ = r.owner.join();
        }
        Ok(())
    }
}

impl Drop for CpalCapture {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn backend(e: impl std::fmt::Display) -> CaptureError {
    CaptureError::Backend(e.to_string())
}

fn map_cpal(e: cpal::Error) -> CaptureError {
    match e.kind() {
        cpal::ErrorKind::PermissionDenied => CaptureError::PermissionDenied("micrófono o audio del sistema"),
        cpal::ErrorKind::UnsupportedOperation => CaptureError::SystemAudioUnavailable,
        _ => CaptureError::Backend(e.to_string()),
    }
}

/// Bloque mono a la frecuencia nativa del stream que lo produjo.
struct Block {
    rate: u32,
    /// Cambia en cada reconstrucción del stream: el DSP rehace el resampler y
    /// comprueba si hay que rellenar un hueco.
    generation: u32,
    samples: Vec<f32>,
}

/// Un canal (mic o sistema): su stream actual, que se puede reconstruir, y su
/// hilo DSP, que vive toda la sesión.
struct ChannelSlot {
    channel: Channel,
    /// Solo mic: id elegido por el usuario. `None` = dispositivo por defecto.
    device_id: Option<String>,
    loopback: bool,
    generation: u32,
    tx: mpsc::SyncSender<Block>,
    stream: Option<cpal::Stream>,
    dsp: JoinHandle<()>,
    epoch: Instant,
    /// Milisegundos desde `epoch` del último callback (o de la última apertura).
    last_activity: Arc<AtomicU64>,
    /// Valor de `last_activity` al abrir el stream actual.
    opened_ms: u64,
    /// Reconstrucciones seguidas por falta de datos (para el backoff).
    stalls: u32,
    /// Pide al hilo DSP que termine. No basta con soltar `tx`: un hilo de
    /// apertura colgado en Core Audio conserva un clon y el DSP no vería nunca
    /// el canal cerrado (Stop se quedaba esperando para siempre).
    closing: Arc<std::sync::atomic::AtomicBool>,
    /// Hay un hilo de apertura sin terminar (colgado en Core Audio): no se
    /// lanza otro hasta que vuelva, para no acumular hilos bloqueados.
    opening: Arc<std::sync::atomic::AtomicBool>,
}

impl ChannelSlot {
    fn new(
        channel: Channel,
        device_id: Option<String>,
        loopback: bool,
        t0: Instant,
        frames: broadcast::Sender<AudioFrame>,
    ) -> Result<Self, CaptureError> {
        let (tx, rx) = mpsc::sync_channel::<Block>(CALLBACK_QUEUE);
        let closing = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let dsp_closing = closing.clone();
        let dsp = std::thread::Builder::new()
            .name(format!("audio-dsp-{}", label(channel)))
            .spawn(move || dsp_thread(channel, t0, rx, frames, dsp_closing))
            .map_err(backend)?;
        Ok(Self {
            channel,
            device_id,
            loopback,
            generation: 0,
            tx,
            stream: None,
            dsp,
            epoch: Instant::now(),
            last_activity: Arc::new(AtomicU64::new(0)),
            opened_ms: 0,
            stalls: 0,
            closing,
            opening: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// (Re)abre el stream del canal. Si `play` es falso queda creado pero en pausa.
    ///
    /// Todo lo que toca Core Audio va en un hilo auxiliar con tiempo límite:
    /// crear el tap del sistema mientras un auricular Bluetooth cambia de perfil
    /// (A2DP → HFP al abrir su micro) puede quedarse colgado para siempre, y
    /// sin el límite colgaba el arranque entero de la grabación.
    fn open(&mut self, commands: &mpsc::Sender<Command>, play: bool) -> Result<(), CaptureError> {
        if self.opening.load(Ordering::Acquire) {
            return Err(CaptureError::Backend(format!("la apertura anterior de {} sigue colgada", label(self.channel))));
        }
        // Soltar el anterior primero: en macOS destruye su tap y aggregate device.
        self.stream = None;
        self.generation = self.generation.wrapping_add(1);
        self.touch();
        self.opened_ms = self.last_activity.load(Ordering::Relaxed);
        let ctx = StreamCtx {
            epoch: self.epoch,
            last_activity: self.last_activity.clone(),
            channel: self.channel,
            generation: self.generation,
            tx: self.tx.clone(),
            commands: commands.clone(),
        };
        let (loopback, device_id, channel) = (self.loopback, self.device_id.clone(), self.channel);
        let (done_tx, done_rx) = mpsc::channel();
        let opening = self.opening.clone();
        opening.store(true, Ordering::Release);
        std::thread::Builder::new()
            .name(format!("audio-open-{}", label(channel)))
            .spawn(move || {
                let res = open_stream(loopback, device_id.as_deref(), ctx, play);
                opening.store(false, Ordering::Release);
                // Si ya se agotó el tiempo nadie escucha: el stream se suelta aquí.
                let _ = done_tx.send(res);
            })
            .map_err(|e| {
                self.opening.store(false, Ordering::Release);
                backend(e)
            })?;
        let timeout = if self.loopback { OPEN_TIMEOUT_SYSTEM } else { OPEN_TIMEOUT_MIC };
        match done_rx.recv_timeout(timeout) {
            Ok(res) => {
                self.stream = Some(res?);
                Ok(())
            }
            Err(_) => {
                Err(CaptureError::Backend(format!("abrir el stream de {} tardó más de {} s", label(self.channel), timeout.as_secs())))
            }
        }
    }
}

fn resolve_device(host: &cpal::Host, loopback: bool, device_id: Option<&str>) -> Result<cpal::Device, CaptureError> {
    if loopback {
        return host.default_output_device().ok_or_else(|| CaptureError::DeviceNotFound("salida por defecto".into()));
    }
    match device_id {
        Some(id) => {
            let parsed = id.parse().map_err(|_| CaptureError::DeviceNotFound(id.to_string()))?;
            host.device_by_id(&parsed).ok_or_else(|| CaptureError::DeviceNotFound(id.to_string()))
        }
        None => host.default_input_device().ok_or_else(|| CaptureError::DeviceNotFound("micrófono por defecto".into())),
    }
}

fn open_stream(loopback: bool, device_id: Option<&str>, ctx: StreamCtx, play: bool) -> Result<cpal::Stream, CaptureError> {
    let host = cpal::default_host();
    let device = resolve_device(&host, loopback, device_id)?;
    let supported = if loopback { device.default_output_config() } else { device.default_input_config() }.map_err(map_cpal)?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let name = device.description().map(|d| d.name().to_string()).unwrap_or_default();
    tracing::info!(
        channel = label(ctx.channel),
        device = %name,
        rate = config.sample_rate,
        channels = config.channels,
        ?format,
        generation = ctx.generation,
        "abriendo stream"
    );
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, ctx)?,
        SampleFormat::I16 => build::<i16>(&device, config, ctx)?,
        SampleFormat::I32 => build::<i32>(&device, config, ctx)?,
        SampleFormat::U16 => build::<u16>(&device, config, ctx)?,
        other => return Err(CaptureError::Backend(format!("formato de muestra no soportado: {other:?}"))),
    };
    if play {
        stream.play().map_err(map_cpal)?;
    }
    Ok(stream)
}

impl ChannelSlot {
    fn touch(&self) {
        self.last_activity.store(self.epoch.elapsed().as_millis() as u64, Ordering::Relaxed);
    }

    /// `true` si el stream lleva demasiado sin datos. El umbral se dobla con cada
    /// reconstrucción fallida seguida (2, 4, 8, 16, 32 s) y vuelve a 2 s en cuanto
    /// llega audio.
    fn stalled(&mut self) -> bool {
        let last_ms = self.last_activity.load(Ordering::Relaxed);
        if last_ms > self.opened_ms {
            self.stalls = 0;
        }
        let limit = STALL_TIMEOUT * 2u32.pow(self.stalls.min(4));
        let idle = self.epoch.elapsed().saturating_sub(Duration::from_millis(last_ms));
        if self.stream.is_some() && idle > limit {
            self.stalls += 1;
            true
        } else {
            false
        }
    }
}

fn label(channel: Channel) -> &'static str {
    match channel {
        Channel::Mic => "mic",
        Channel::System => "system",
    }
}

fn owner_thread(
    cfg: CaptureConfig,
    frames: broadcast::Sender<AudioFrame>,
    commands_tx: mpsc::Sender<Command>,
    commands: mpsc::Receiver<Command>,
    ready: mpsc::Sender<Result<Instant, CaptureError>>,
) {
    let t0 = Instant::now();
    let mut slots: Vec<ChannelSlot> = Vec::new();

    let setup = (|| -> Result<(), CaptureError> {
        // El mic va primero: si es Bluetooth, abrirlo cambia el perfil y la
        // frecuencia de la salida; el stream del sistema se reconstruye solo.
        if !cfg.capture_mic && !cfg.capture_system_audio {
            return Err(CaptureError::Backend("no hay ningún canal que capturar".into()));
        }
        if cfg.capture_mic {
            let mut mic = ChannelSlot::new(Channel::Mic, cfg.mic_device.clone(), false, t0, frames.clone())?;
            mic.open(&commands_tx, true)?;
            slots.push(mic);
        }
        if cfg.capture_system_audio {
            let mut sys = ChannelSlot::new(Channel::System, None, true, t0, frames.clone())?;
            match sys.open(&commands_tx, true) {
                Ok(()) => {}
                // Permisos o falta de soporte: no tiene arreglo, se informa.
                Err(e @ (CaptureError::PermissionDenied(_) | CaptureError::SystemAudioUnavailable)) => return Err(e),
                // Lo demás (p. ej. Core Audio colgado en un cambio de perfil BT)
                // suele ser pasajero: se graba el mic y el sistema se reintenta.
                Err(e) => {
                    tracing::warn!("audio del sistema: {e}; se reintenta en segundo plano");
                    let _ = commands_tx.send(Command::Rebuild(Channel::System));
                }
            }
            slots.push(sys);
        }
        Ok(())
    })();

    let ok = setup.is_ok();
    let _ = ready.send(setup.map(|()| t0));
    if ok {
        command_loop(&mut slots, &commands_tx, &commands);
    }
    // Soltar streams y emisores termina los hilos DSP.
    let handles: Vec<_> = slots
        .into_iter()
        .map(|s| {
            drop(s.stream);
            drop(s.tx);
            s.closing.store(true, Ordering::Relaxed);
            s.dsp
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
}

fn command_loop(slots: &mut [ChannelSlot], commands_tx: &mpsc::Sender<Command>, commands: &mpsc::Receiver<Command>) {
    let mut paused = false;
    // Canales pendientes de reconstruir y cuándo intentarlo.
    let mut pending: Vec<(Channel, Instant)> = Vec::new();
    loop {
        let timeout =
            pending.iter().map(|(_, at)| at.saturating_duration_since(Instant::now())).min().unwrap_or(WATCHDOG_TICK).min(WATCHDOG_TICK);
        match commands.recv_timeout(timeout) {
            Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Ok(Command::Pause) => {
                paused = true;
                slots.iter().filter_map(|s| s.stream.as_ref()).for_each(|s| {
                    let _ = s.pause();
                });
            }
            Ok(Command::Resume) => {
                paused = false;
                for slot in slots.iter() {
                    slot.touch();
                    if let Some(s) = &slot.stream {
                        let _ = s.play();
                    }
                }
            }
            Ok(Command::Rebuild(ch)) => {
                if !pending.iter().any(|(c, _)| *c == ch) {
                    pending.push((ch, Instant::now() + REBUILD_SETTLE));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if !paused {
            for slot in slots.iter_mut() {
                if !pending.iter().any(|(c, _)| *c == slot.channel) && slot.stalled() {
                    tracing::warn!(channel = label(slot.channel), attempt = slot.stalls, "stream sin datos, se reconstruye");
                    pending.push((slot.channel, Instant::now()));
                }
            }
        }
        let now = Instant::now();
        let due: Vec<Channel> = pending.iter().filter(|(_, at)| *at <= now).map(|(c, _)| *c).collect();
        pending.retain(|(_, at)| *at > now);
        for ch in due {
            let Some(slot) = slots.iter_mut().find(|s| s.channel == ch) else { continue };
            match slot.open(commands_tx, !paused) {
                Ok(()) => tracing::info!(channel = label(ch), "stream reconstruido"),
                Err(e) => {
                    tracing::warn!(channel = label(ch), "no se pudo reconstruir: {e}; reintento");
                    pending.push((ch, Instant::now() + REBUILD_RETRY));
                }
            }
        }
    }
}

fn dsp_thread(
    channel: Channel,
    t0: Instant,
    rx: mpsc::Receiver<Block>,
    frames: broadcast::Sender<AudioFrame>,
    closing: Arc<std::sync::atomic::AtomicBool>,
) {
    let rate = TARGET_SAMPLE_RATE as f64;
    let mut resampler: Option<MonoResampler> = None;
    let mut generation = None;
    let mut check_gap = false;
    // Todos los canales cuentan desde t0: el timestamp de una muestra es
    // `emitted / 16 kHz`, y los huecos (arranque tardío, reconstrucciones) se
    // rellenan con silencio para que los WAV de mic y sistema queden alineados.
    let mut emitted: u64 = 0;

    let emit = |samples: Vec<f32>, emitted: &mut u64| {
        let timestamp = *emitted as f64 / rate;
        *emitted += samples.len() as u64;
        // Sin receptores no es un error: seguimos para no bloquear el callback.
        let _ = frames.send(AudioFrame { channel, samples, timestamp });
    };

    loop {
        // Con la cola vacía y el cierre pedido, se termina aunque quede algún
        // emisor vivo (ver `ChannelSlot::closing`).
        let block = match rx.recv_timeout(DSP_POLL) {
            Ok(b) => b,
            Err(mpsc::RecvTimeoutError::Timeout) if !closing.load(Ordering::Relaxed) => continue,
            Err(_) => break,
        };
        if generation != Some(block.generation) {
            generation = Some(block.generation);
            match MonoResampler::new(block.rate) {
                Ok(r) => resampler = Some(r),
                Err(e) => {
                    tracing::error!(channel = label(channel), "resampler: {e}");
                    resampler = None;
                }
            }
            check_gap = true;
        }
        let Some(r) = resampler.as_mut() else { continue };
        let out = r.push(&block.samples);
        if out.is_empty() {
            continue;
        }
        if check_gap {
            check_gap = false;
            // Instante aproximado de la primera muestra de `out`.
            let now = t0.elapsed().as_secs_f64() - out.len() as f64 / rate;
            let gap = now - emitted as f64 / rate;
            if gap > GAP_TOLERANCE_SECS {
                // Acotado: un hueco de horas (suspensión, reconstrucción lenta)
                // no puede reservar cientos de MB de ceros de golpe.
                let fill = (gap.min(60.0) * rate) as usize;
                tracing::debug!(channel = label(channel), gap, fill_secs = 60.0_f32.min(gap as f32), "relleno de silencio");
                emit(vec![0.0; fill], &mut emitted);
            }
        }
        emit(out, &mut emitted);
    }
}

struct StreamCtx {
    channel: Channel,
    generation: u32,
    tx: mpsc::SyncSender<Block>,
    commands: mpsc::Sender<Command>,
    epoch: Instant,
    last_activity: Arc<AtomicU64>,
}

fn build<T>(device: &cpal::Device, config: cpal::StreamConfig, ctx: StreamCtx) -> Result<cpal::Stream, CaptureError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let n = config.channels.max(1) as usize;
    let rate = config.sample_rate;
    let StreamCtx { channel, generation, tx, commands, epoch, last_activity } = ctx;
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                // Downmix a mono en el callback para mover 1/n de datos.
                let samples: Vec<f32> =
                    data.chunks_exact(n).map(|frame| frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / n as f32).collect();
                // try_send: nunca bloquear el hilo de tiempo real.
                last_activity.store(epoch.elapsed().as_millis() as u64, Ordering::Relaxed);
                if tx.try_send(Block { rate, generation, samples }).is_err() {
                    tracing::warn!(channel = label(channel), "cola de audio llena, se descarta un bloque");
                }
            },
            move |err: cpal::Error| {
                tracing::warn!(channel = label(channel), "error de stream: {err}");
                if matches!(
                    err.kind(),
                    cpal::ErrorKind::StreamInvalidated | cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::DeviceNotAvailable
                ) {
                    let _ = commands.send(Command::Rebuild(channel));
                }
            },
            None,
        )
        .map_err(map_cpal)
}
