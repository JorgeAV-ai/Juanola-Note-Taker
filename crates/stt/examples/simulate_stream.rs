//! Reproduce un WAV en tiempo real contra `juanola-stt-worker` y mide el
//! retraso de cada resultado: cuánto después del final del tramo de audio
//! aparece su texto.
//!
//! ```text
//! cargo build --release -p juanola-stt --features mlx --bins --examples
//! target/release/examples/simulate_stream <wav> [idioma|auto] [mic|system]
//! ```

use std::path::PathBuf;
use std::time::{Duration, Instant};

use juanola_core_types::Channel;
use juanola_stt::worker::{SttWorker, WorkerConfig, WorkerEvent};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let wav = args.first().ok_or_else(|| anyhow::anyhow!("uso: simulate_stream <wav> [idioma|auto] [mic|system]"))?;
    let language = match args.get(1).map(String::as_str) {
        Some("auto") => None,
        Some(l) => Some(l.to_owned()),
        None => Some("Spanish".to_owned()),
    };
    let channel = if args.get(2).map(String::as_str) == Some("system") { Channel::System } else { Channel::Mic };

    let mut reader = hound::WavReader::open(wav)?;
    anyhow::ensure!(reader.spec().sample_rate == 16_000 && reader.spec().channels == 1, "WAV mono 16 kHz");
    let samples: Vec<f32> = reader.samples::<i16>().map(|s| s.map(|v| v as f32 / 32768.0)).collect::<Result<_, _>>()?;

    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let models = home.join("Library/Application Support/dev.juanola.desktop/models");
    // El worker está en target/<perfil>/; este ejemplo en target/<perfil>/examples/.
    let program = std::env::current_exe()?.parent().and_then(|p| p.parent()).unwrap().join("juanola-stt-worker");
    let mut worker = SttWorker::spawn(&WorkerConfig {
        program,
        model_dir: models.join("stt_streaming/qwen3-asr-0.6b-mlx"),
        vad_dir: models.join("vad/silero-vad-mlx"),
        language,
    })?;
    match worker.events().recv()? {
        WorkerEvent::Ready { load_ms } => println!("worker listo en {load_ms} ms"),
        other => anyhow::bail!("esperaba ready, llegó {other:?}"),
    }

    // Enviar en bloques de 20 ms al ritmo real.
    let started = Instant::now();
    const BLOCK: usize = 320;
    let mut lags = Vec::new();
    for (i, block) in samples.chunks(BLOCK).enumerate() {
        let due = Duration::from_secs_f64((i * BLOCK) as f64 / 16_000.0);
        if let Some(wait) = due.checked_sub(started.elapsed()) {
            std::thread::sleep(wait);
        }
        worker.send(channel, block)?;
        for ev in worker.events().try_iter() {
            report(&ev, started, &mut lags);
        }
    }
    // Un poco de silencio para cerrar la última frase y fin de la entrada.
    worker.send(channel, &vec![0.0; 16_000])?;
    worker.finish();
    while let Ok(ev) = worker.events().recv_timeout(Duration::from_secs(10)) {
        report(&ev, started, &mut lags);
    }
    if !lags.is_empty() {
        lags.sort_by(f64::total_cmp);
        let p = |q: f64| lags[((lags.len() - 1) as f64 * q).round() as usize];
        println!("retraso texto vs. fin del tramo: mediana {:.2}s, p90 {:.2}s, máx {:.2}s ({} resultados)", p(0.5), p(0.9), p(1.0), lags.len());
    }
    Ok(())
}

fn report(ev: &WorkerEvent, started: Instant, lags: &mut Vec<f64>) {
    match ev {
        WorkerEvent::Segment { id, start, end, text, is_final, decode_ms, .. } => {
            let lag = started.elapsed().as_secs_f64() - end;
            lags.push(lag);
            println!(
                "{:>6.2}s  #{id} {} [{start:>5.1}-{end:>5.1}] retraso {lag:.2}s (decode {decode_ms} ms)  {text}",
                started.elapsed().as_secs_f64(),
                if *is_final { "FINAL  " } else { "parcial" },
            );
        }
        other => println!("{other:?}"),
    }
}
