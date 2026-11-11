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
use juanola_stt::worker::{read_frame, WorkerEvent, TARGET_SAMPLE_RATE};

struct ChannelState {
    channel: Channel,
    vad: SileroMlx,
    seg: Segmenter,
    /// Audio desde `buf_start` (muestra absoluta del canal).
    buf: Vec<f32>,
    buf_start: u64,
    /// Siguiente muestra que pasa por el VAD.
    vad_pos: u64,
    next_id: u64,
    pending: Vec<Action>,
}

impl ChannelState {
    fn end(&self) -> u64 {
        self.buf_start + self.buf.len() as u64
    }

    fn push(&mut self, samples: &[f32]) -> Result<(), String> {
        self.buf.extend_from_slice(samples);
        while self.end() - self.vad_pos >= WINDOW as u64 {
            let i = (self.vad_pos - self.buf_start) as usize;
            let p = self.vad.prob(&self.buf[i..i + WINDOW]).map_err(|e| e.to_string())?;
            self.vad_pos += WINDOW as u64;
            if let Some(a) = self.seg.push(p, WINDOW as u64, self.vad_pos) {
                self.pending.push(a);
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
    std::thread::spawn(move || {
        let mut stdin = std::io::BufReader::new(std::io::stdin().lock());
        loop {
            match read_frame(&mut stdin) {
                Ok(Some(f)) => {
                    if tx.try_send(Some(f)).is_err() {
                        eprintln!("juanola-stt-worker: cola llena, se descarta una trama de audio");
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
                    // Un fallo de VAD/MLX en una ventana no tumba la sesión:
                    // se avisa y se salta esa trama, como los errores del ASR.
                    match channels.iter_mut().find(|c| c.channel == channel) {
                        Some(ch) => {
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
                    start: span.start as f64 / TARGET_SAMPLE_RATE,
                    end: span.end as f64 / TARGET_SAMPLE_RATE,
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
