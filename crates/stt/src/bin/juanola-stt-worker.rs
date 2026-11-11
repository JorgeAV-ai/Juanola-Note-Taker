//! Worker de transcripción en streaming: Silero VAD + Qwen3-ASR sobre MLX.
//! Protocolo en `juanola_stt::worker`.
//!
//! ```text
//! juanola-stt-worker --model <dir> --vad <dir> [--language Spanish|auto] [--context <texto>]
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use juanola_core_types::Channel;
use juanola_stt::backends::qwen3_mlx::Qwen3AsrMlx;
use juanola_stt::backends::silero_mlx::{SileroMlx, WINDOW};
use juanola_stt::language_lock::LanguageLock;
use juanola_stt::segmenter::{Action, Segmenter, SegmenterConfig, Span};
use juanola_stt::worker::{channel_byte, read_frame, WorkerEvent, TARGET_SAMPLE_RATE};

struct ChannelState {
    channel: Channel,
    vad: SileroMlx,
    seg: Segmenter,
    /// Audio desde `buf_start` (muestra absoluta del canal).
    buf: Vec<f32>,
    buf_start: u64,
    /// Siguiente muestra que pasa por el VAD.
    vad_pos: u64,
    /// El VAD falló de forma persistente (típicamente Metal): se deja de
    /// segmentar el canal pero se sigue consumiendo el búfer para que `trim`
    /// pueda drenarlo y la sesión no se coma la memoria.
    vad_poisoned: bool,
    /// Huecos por tramas descartadas con la cola llena: posición del canal
    /// donde cayó el hueco y muestras perdidas. Las posiciones internas no los
    /// cuentan, así que `abs` los suma al publicar tiempos.
    skips: Vec<(u64, u64)>,
    next_id: u64,
    pending: Vec<Action>,
}

impl ChannelState {
    fn end(&self) -> u64 {
        self.buf_start + self.buf.len() as u64
    }

    /// Posición real en el reloj del canal: el búfer solo cuenta lo recibido,
    /// pero los huecos de tramas descartadas también pasaron.
    /// `inclusive = false` es para límites de fin exclusivos: un hueco justo en
    /// la posición empieza DESPUÉS del tramo, así que no se suma.
    fn abs(&self, pos: u64, inclusive: bool) -> u64 {
        pos + self.skips.iter().filter(|(h, _)| if inclusive { *h <= pos } else { *h < pos }).map(|(_, c)| c).sum::<u64>()
    }

    fn push(&mut self, samples: &[f32]) -> Result<(), String> {
        self.buf.extend_from_slice(samples);
        while self.end() - self.vad_pos >= WINDOW as u64 {
            if self.vad_poisoned {
                self.vad_pos += WINDOW as u64;
                continue;
            }
            let i = (self.vad_pos - self.buf_start) as usize;
            match self.vad.prob(&self.buf[i..i + WINDOW]) {
                Ok(p) => {
                    self.vad_pos += WINDOW as u64;
                    if let Some(a) = self.seg.push(p, WINDOW as u64, self.vad_pos) {
                        self.pending.push(a);
                    }
                }
                Err(e) => {
                    // Un fallo de VAD/MLX no tumba la sesión: se cierra la
                    // frase abierta (si la hay) para que `trim` no quede
                    // anclado, se deja de segmentar el canal y se avisa una vez.
                    self.vad_poisoned = true;
                    self.vad_pos += WINDOW as u64;
                    if let Some(a) = self.seg.flush(self.vad_pos) {
                        self.pending.push(a);
                    }
                    return Err(e.to_string());
                }
            }
        }
        Ok(())
    }

    fn audio(&self, span: Span) -> &[f32] {
        // Defensivo: si el tramo quedara fuera del búfer (p. ej. un `trim`
        // desplazado), devolver lo que quede en vez de reventar el worker.
        let from = (span.start.max(self.buf_start) - self.buf_start) as usize;
        let to = (span.end.min(self.end()).saturating_sub(self.buf_start)) as usize;
        &self.buf[from.min(self.buf.len())..to.clamp(from.min(self.buf.len()), self.buf.len())]
    }

    /// Descarta el audio que ya no puede formar parte de ningún tramo.
    fn trim(&mut self) {
        if !self.pending.is_empty() {
            return;
        }
        let keep = self.seg.keep_from(self.vad_pos).max(self.buf_start);
        let n = (keep - self.buf_start) as usize;
        self.buf.drain(..n.min(self.buf.len()));
        self.buf_start = keep;
    }
}

fn emit(ev: &WorkerEvent) {
    let mut out = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut out, ev);
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    if let Err(e) = run() {
        emit(&WorkerEvent::Error { message: e });
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let model_dir = PathBuf::from(arg(&args, "--model").ok_or("falta --model")?);
    let vad_dir = PathBuf::from(arg(&args, "--vad").ok_or("falta --vad")?);
    let language = arg(&args, "--language").filter(|l| l != "auto");

    let t = Instant::now();
    let mut asr = Qwen3AsrMlx::load(&model_dir).map_err(|e| e.to_string())?;
    if let Some(c) = arg(&args, "--context") {
        asr.context = c;
    }
    let mut channels = Vec::new();
    for channel in [Channel::Mic, Channel::System] {
        channels.push(ChannelState {
            channel,
            vad: SileroMlx::load(&vad_dir).map_err(|e| e.to_string())?,
            seg: Segmenter::new(SegmenterConfig::default()),
            buf: Vec::new(),
            buf_start: 0,
            vad_pos: 0,
            vad_poisoned: false,
            skips: Vec::new(),
            next_id: 0,
            pending: Vec::new(),
        });
    }
    // Calentar los kernels de Metal para que la primera frase no pague el JIT.
    let _ = asr.transcribe(&vec![0.0; 16_000], language.as_deref());
    emit(&WorkerEvent::Ready { load_ms: t.elapsed().as_millis() as u64 });

    // Cola acotada: si la decodificación va por detrás de la captura durante
    // mucho tiempo, las tramas se descartan en vez de crecer sin límite. Las
    // tramas son de ~10–20 ms por canal: esto deja minutos de margen (un
    // decode largo no debe perder audio) con unos pocos MB como mucho.
    const MAX_QUEUED_FRAMES: usize = 12_000;
    let (tx, rx) = mpsc::sync_channel::<Option<(Channel, Vec<f32>)>>(MAX_QUEUED_FRAMES);
    // Muestras encoladas y huecos descartados por canal: el reloj de cada
    // canal es lo que llegó a la cola, así que cada descarte queda anclado a
    // la posición donde ocurrió (no a la que vaya procesando el bucle).
    #[derive(Default)]
    struct ChanClock {
        sent: u64,
        drops: Vec<(u64, u64)>,
    }
    let clock: [std::sync::Arc<std::sync::Mutex<ChanClock>>; 2] = [
        std::sync::Arc::new(std::sync::Mutex::new(ChanClock::default())),
        std::sync::Arc::new(std::sync::Mutex::new(ChanClock::default())),
    ];
    let reader_clock = clock.clone();
    std::thread::spawn(move || {
        let mut stdin = std::io::BufReader::new(std::io::stdin().lock());
        let mut warned = 0u64;
        loop {
            match read_frame(&mut stdin) {
                Ok(Some((ch, samples))) => {
                    let n = samples.len() as u64;
                    match tx.try_send(Some((ch, samples))) {
                        Ok(()) => {
                            reader_clock[channel_byte(ch) as usize].lock().unwrap_or_else(|e| e.into_inner()).sent += n;
                        }
                        Err(_) => {
                            let mut c = reader_clock[channel_byte(ch) as usize].lock().unwrap_or_else(|e| e.into_inner());
                            let at = c.sent;
                            c.drops.push((at, n));
                            warned += 1;
                            if warned == 1 || warned % 200 == 0 {
                                eprintln!("juanola-stt-worker: cola llena, {warned} tramas de audio descartadas");
                            }
                        }
                    }
                }
                Ok(None) => {
                    eprintln!("juanola-stt-worker: fin de la entrada");
                    let _ = tx.send(None);
                    return;
                }
                Err(e) => {
                    eprintln!("juanola-stt-worker: entrada no válida ({e}); se termina");
                    let _ = tx.send(None);
                    return;
                }
            }
        }
    });

    // Con «auto», el idioma se fija en cuanto uno domina (ver `LanguageLock`).
    let mut lock = LanguageLock::default();
    let mut eof = false;
    while !eof {
        // Bloquear por la primera trama y luego vaciar la cola: si la
        // decodificación va por detrás, los parciales intermedios se saltan.
        let Ok(first) = rx.recv() else { break };
        let mut batch = vec![first];
        batch.extend(rx.try_iter());
        for item in batch {
            match item {
                Some((channel, samples)) => {
                    match channels.iter_mut().find(|c| c.channel == channel) {
                        Some(ch) => {
                            // Las tramas que la cola llena descartó dejan un
                            // hueco en el reloj del canal: se anotan en su
                            // posición real para que los tiempos de los
                            // segmentos sigan siendo reales.
                            let drops =
                                std::mem::take(&mut clock[channel_byte(channel) as usize].lock().unwrap_or_else(|e| e.into_inner()).drops);
                            ch.skips.extend(drops);
                            // Un fallo de VAD/MLX en una ventana no tumba la
                            // sesión: se avisa y el canal queda envenenado.
                            if let Err(e) = ch.push(&samples) {
                                emit(&WorkerEvent::Error { message: format!("{channel:?}: {e}") });
                            }
                        }
                        None => emit(&WorkerEvent::Error { message: format!("canal desconocido: {channel:?}") }),
                    }
                }
                None => eof = true,
            }
        }
        if eof {
            for ch in channels.iter_mut() {
                let pos = ch.vad_pos;
                if let Some(a) = ch.seg.flush(pos) {
                    ch.pending.push(a);
                }
            }
        }
        for ch in channels.iter_mut() {
            run_pending(&asr, ch, language.as_deref(), &mut lock);
            ch.trim();
        }
    }
    Ok(())
}

fn run_pending(asr: &Qwen3AsrMlx, ch: &mut ChannelState, fixed: Option<&str>, lock: &mut LanguageLock) {
    let actions = std::mem::take(&mut ch.pending);
    let last = actions.len().saturating_sub(1);
    for (i, action) in actions.into_iter().enumerate() {
        let (span, is_final) = match action {
            // Un parcial con algo detrás ya está obsoleto.
            Action::Partial(_) if i < last => continue,
            Action::Partial(s) => (s, false),
            Action::Final(s) => (s, true),
        };
        let t = Instant::now();
        let language = fixed.or(lock.locked());
        let ev = match asr.transcribe(ch.audio(span), language) {
            Ok(out) => {
                if is_final && language.is_none() {
                    if let Some(l) = out.language.as_deref() {
                        let secs = (span.end - span.start) as f64 / TARGET_SAMPLE_RATE;
                        if let Some(l) = lock.observe(l, secs) {
                            eprintln!("juanola-stt-worker: idioma fijado en {l}");
                        }
                    }
                }
                if !is_final && out.text.is_empty() {
                    continue;
                }
                WorkerEvent::Segment {
                    channel: ch.channel,
                    id: ch.next_id,
                    start: ch.abs(span.start, true) as f64 / TARGET_SAMPLE_RATE,
                    end: ch.abs(span.end, false) as f64 / TARGET_SAMPLE_RATE,
                    confidence: out.min_prob(),
                    token_probs: if is_final { out.token_probs } else { Vec::new() },
                    token_texts: if is_final { out.token_texts } else { Vec::new() },
                    text: out.text,
                    is_final,
                    language: out.language,
                    decode_ms: t.elapsed().as_millis() as u64,
                }
            }
            Err(e) => WorkerEvent::Error { message: e.to_string() },
        };
        emit(&ev);
        if is_final {
            ch.next_id += 1;
        }
    }
}
