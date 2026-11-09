//! Prueba técnica 2 en vivo: captura mic + sistema (`juanola-audio-capture`) y
//! transcribe ambos canales con `juanola-stt-worker`.
//!
//! ```text
//! crates/stt/scripts/live-app.sh [segundos] [idioma|auto]
//! ```
//!
//! Imprime cada resultado con su canal (Me = micrófono, Them = sistema) y el
//! retraso respecto al final del tramo de audio.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use juanola_audio_capture::{AudioCapture, CaptureConfig, CpalCapture};
use juanola_core_types::Channel;
use juanola_stt::worker::{SttWorker, WorkerConfig, WorkerEvent};
use tokio::sync::broadcast::error::RecvError;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let limit = args.first().and_then(|s| s.parse::<u64>().ok()).map(Duration::from_secs);
    let language = match args.get(1).map(String::as_str) {
        Some("auto") => None,
        Some(l) => Some(l.to_owned()),
        None => Some("Spanish".to_owned()),
    };

    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let models = home.join("Library/Application Support/dev.juanola.desktop/models");
    let program = std::env::var_os("JUANOLA_STT_WORKER").map(PathBuf::from).unwrap_or_else(|| {
        let exe = std::env::current_exe().expect("exe");
        let dir = exe.parent().unwrap();
        // Dentro de la .app el worker va al lado; en target/, un nivel arriba.
        let side = dir.join("juanola-stt-worker");
        if side.exists() { side } else { dir.parent().unwrap().join("juanola-stt-worker") }
    });
    let mut worker = SttWorker::spawn(&WorkerConfig {
        program,
        model_dir: models.join("stt_streaming/qwen3-asr-1.7b-4bit-mlx"),
        vad_dir: models.join("vad/silero-vad-mlx"),
        language,
        context: None,
    })?;
    match worker.events().recv()? {
        WorkerEvent::Ready { load_ms } => println!("STT listo en {load_ms} ms"),
        other => anyhow::bail!("esperaba ready, llegó {other:?}"),
    }

    let mut capture = CpalCapture::new();
    let mut rx = capture.start(CaptureConfig { mic_device: None, capture_mic: true, capture_system_audio: true, echo_cancellation: false })?;
    // Medir contra el origen de los timestamps de la captura, no contra la
    // vuelta de start() (abrir un micro Bluetooth tarda segundos).
    let started = capture.session_start().unwrap_or_else(Instant::now);
    println!("Escuchando… (Ctrl+C para parar)");
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    let mut lags = Vec::new();
    // Muestras recibidas por canal y primer timestamp, para vigilar la deriva
    // entre el reloj del audio y el de pared.
    let mut sent = [0u64; 2];
    let mut first_ts: [Option<(f64, f64)>; 2] = [None, None];
    let mut last_report = Instant::now();

    loop {
        if limit.is_some_and(|l| started.elapsed() >= l) {
            break;
        }
        tokio::select! {
            _ = &mut ctrl_c => break,
            _ = tokio::time::sleep(Duration::from_millis(50)) => {}
            frame = rx.recv() => match frame {
                Ok(f) => {
                    let i = if f.channel == Channel::Mic { 0 } else { 1 };
                    first_ts[i].get_or_insert((f.timestamp, started.elapsed().as_secs_f64()));
                    sent[i] += f.samples.len() as u64;
                    worker.send(f.channel, &f.samples)?;
                }
                Err(RecvError::Lagged(n)) => eprintln!("aviso: {n} frames de audio perdidos"),
                Err(RecvError::Closed) => break,
            },
        }
        for ev in worker.events().try_iter() {
            print_event(&ev, started, &mut lags);
        }
        if std::env::var_os("JUANOLA_CLOCK_DEBUG").is_some() && last_report.elapsed() >= Duration::from_secs(5) {
            last_report = Instant::now();
            println!(
                "[reloj] pared {:.1}s | audio mic {:.1}s sys {:.1}s | primer frame mic {:?} sys {:?}",
                started.elapsed().as_secs_f64(),
                sent[0] as f64 / 16_000.0,
                sent[1] as f64 / 16_000.0,
                first_ts[0],
                first_ts[1],
            );
        }
    }

    capture.stop()?;
    worker.finish();
    while let Ok(ev) = worker.events().recv_timeout(Duration::from_secs(10)) {
        print_event(&ev, started, &mut lags);
    }
    if !lags.is_empty() {
        lags.sort_by(f64::total_cmp);
        let p = |q: f64| lags[((lags.len() - 1) as f64 * q).round() as usize];
        println!("retraso: mediana {:.2}s, p90 {:.2}s, máx {:.2}s ({} resultados)", p(0.5), p(0.9), p(1.0), lags.len());
    }
    Ok(())
}

fn print_event(ev: &WorkerEvent, started: Instant, lags: &mut Vec<f64>) {
    match ev {
        WorkerEvent::Segment { channel, id, end, text, is_final, .. } => {
            // El reloj de la captura empieza a la vez que `started` (± unos ms).
            let lag = started.elapsed().as_secs_f64() - end;
            lags.push(lag);
            let who = match channel {
                Channel::Mic => "Me  ",
                Channel::System => "Them",
            };
            let kind = if *is_final { "■" } else { "…" };
            println!("{:>6.1}s {who} #{id:<3}{kind} (+{lag:.2}s) {text}", started.elapsed().as_secs_f64());
        }
        other => println!("{other:?}"),
    }
}
