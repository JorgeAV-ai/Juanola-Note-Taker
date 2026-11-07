//! Prueba técnica 1: graba micrófono y audio del sistema en dos WAV a 16 kHz mono.
//!
//! ```text
//! cargo run -p juanola-audio-capture --example record_dual -- [segundos] [carpeta]
//! ```
//!
//! Por defecto graba hasta Ctrl+C y escribe `mic.wav` y `system.wav` en `./recordings`.
//! Imprime los niveles RMS de cada canal dos veces por segundo.
//! Para listar micrófonos: `--list`. Para elegir uno: `JUANOLA_MIC=<id>`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use juanola_audio_capture::{rms, AudioCapture, CaptureConfig, CpalCapture, TARGET_SAMPLE_RATE};
use juanola_core_types::Channel;
use tokio::sync::broadcast::error::RecvError;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut capture = CpalCapture::new();
    if args.first().map(String::as_str) == Some("--list") {
        for d in capture.list_input_devices()? {
            println!("{}{}  [{}]", if d.is_default { "* " } else { "  " }, d.name, d.id);
        }
        return Ok(());
    }
    let limit = args.first().and_then(|s| s.parse::<u64>().ok()).map(Duration::from_secs);
    let dir = PathBuf::from(args.get(1).cloned().unwrap_or_else(|| "recordings".into()));
    std::fs::create_dir_all(&dir)?;

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut mic_wav = hound::WavWriter::create(dir.join("mic.wav"), spec)?;
    let mut sys_wav = hound::WavWriter::create(dir.join("system.wav"), spec)?;

    let mut rx = capture.start(CaptureConfig {
        mic_device: std::env::var("JUANOLA_MIC").ok(),
        capture_system_audio: true,
        echo_cancellation: false,
    })?;
    println!("Grabando en {} … (Ctrl+C para parar)", dir.display());

    let started = Instant::now();
    let mut last_print = Instant::now();
    let (mut mic_acc, mut sys_acc) = (Vec::<f32>::new(), Vec::<f32>::new());
    let (mut mic_total, mut sys_total, mut mic_peak, mut sys_peak) = (0usize, 0usize, 0f32, 0f32);
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    loop {
        if limit.is_some_and(|l| started.elapsed() >= l) {
            break;
        }
        tokio::select! {
            _ = &mut ctrl_c => break,
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            frame = rx.recv() => match frame {
                Ok(f) => {
                    let (wav, acc, total) = match f.channel {
                        Channel::Mic => (&mut mic_wav, &mut mic_acc, &mut mic_total),
                        Channel::System => (&mut sys_wav, &mut sys_acc, &mut sys_total),
                    };
                    for s in &f.samples {
                        wav.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
                    }
                    let peak = match f.channel {
                        Channel::Mic => &mut mic_peak,
                        Channel::System => &mut sys_peak,
                    };
                    *peak = f.samples.iter().fold(*peak, |m, s| m.max(s.abs()));
                    *total += f.samples.len();
                    acc.extend_from_slice(&f.samples);
                }
                Err(RecvError::Lagged(n)) => tracing::warn!("consumidor lento: {n} frames perdidos"),
                Err(RecvError::Closed) => break,
            },
        }
        if last_print.elapsed() >= Duration::from_millis(500) {
            println!(
                "{:>6.1}s  mic {:<20} system {:<20}",
                started.elapsed().as_secs_f32(),
                bar(rms(&mic_acc)),
                bar(rms(&sys_acc)),
            );
            mic_acc.clear();
            sys_acc.clear();
            // Reescribe la cabecera: el WAV queda legible aunque el proceso muera.
            mic_wav.flush()?;
            sys_wav.flush()?;
            last_print = Instant::now();
        }
    }

    capture.stop()?;
    mic_wav.finalize()?;
    sys_wav.finalize()?;
    let secs = |n: usize| n as f32 / TARGET_SAMPLE_RATE as f32;
    println!(
        "Listo: mic.wav {:.1}s (pico {:.3}), system.wav {:.1}s (pico {:.3})",
        secs(mic_total),
        mic_peak,
        secs(sys_total),
        sys_peak
    );
    if mic_total > 0 && mic_peak == 0.0 {
        println!(
            "Aviso: mic.wav es cero absoluto, lo que suele indicar permiso denegado. Revisa \
             Ajustes del Sistema > Privacidad y seguridad > Micrófono para la app de terminal."
        );
    }
    if sys_total > 0 && sys_peak == 0.0 {
        println!(
            "Aviso: system.wav es silencio puro. Si sonaba algo, revisa Ajustes del Sistema > \
             Privacidad y seguridad > Grabación de pantalla y audio del sistema > \
             «Solo audio del sistema» para la app de terminal."
        );
    }
    Ok(())
}

fn bar(level: f32) -> String {
    // Escala aproximada en dB: -60 dB → 0, 0 dB → 20 celdas.
    let db = 20.0 * level.max(1e-6).log10();
    let cells = (((db + 60.0) / 60.0).clamp(0.0, 1.0) * 20.0) as usize;
    "█".repeat(cells)
}
