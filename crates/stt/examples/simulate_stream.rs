//! Reproduce un WAV (o dos: micrófono y sistema a la vez, como en una
//! reunión) en tiempo real contra `juanola-stt-worker` y mide el retraso de
//! cada resultado: cuánto después del final del tramo de audio aparece su texto.
//!
//! ```text
//! cargo build --release -p juanola-stt --features mlx --bins --examples
//! target/release/examples/simulate_stream <wav> [idioma|auto] [mic|system]
//! target/release/examples/simulate_stream <mic.wav> <system.wav> [idioma|auto]
//! ```
//!
//! `JUANOLA_ASR_MODEL` cambia la carpeta del modelo. `-q` solo imprime el resumen.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use juanola_core_types::Channel;
use juanola_stt::worker::{SttWorker, WorkerConfig, WorkerEvent};

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let quiet = args.iter().any(|a| a == "-q");
    args.retain(|a| a != "-q");
    let wav = args.first().ok_or_else(|| anyhow::anyhow!("uso: simulate_stream <wav> [idioma|auto] [mic|system]"))?;
    let dual = args.get(1).is_some_and(|a| a.ends_with(".wav"));
    let lang_arg = if dual { args.get(2) } else { args.get(1) };
    let language = match lang_arg.map(String::as_str) {
        Some("auto") => None,
        Some(l) => Some(l.to_owned()),
        None => Some("Spanish".to_owned()),
    };
    let read = |path: &str| -> anyhow::Result<Vec<f32>> {
        let mut reader = hound::WavReader::open(path)?;
        anyhow::ensure!(reader.spec().sample_rate == 16_000 && reader.spec().channels == 1, "WAV mono 16 kHz");
        Ok(reader.samples::<i16>().map(|s| s.map(|v| v as f32 / 32768.0)).collect::<Result<_, _>>()?)
    };
    let tracks: Vec<(Channel, Vec<f32>)> = if dual {
        vec![(Channel::Mic, read(wav)?), (Channel::System, read(&args[1])?)]
    } else {
        let channel = if args.get(2).map(String::as_str) == Some("system") { Channel::System } else { Channel::Mic };
        vec![(channel, read(wav)?)]
    };

    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let models = home.join("Library/Application Support/dev.juanola.desktop/models");
    // El worker está en target/<perfil>/; este ejemplo en target/<perfil>/examples/.
    let program = std::env::current_exe()?.parent().and_then(|p| p.parent()).unwrap().join("juanola-stt-worker");
    let mut worker = SttWorker::spawn(&WorkerConfig {
        program,
        model_dir: std::env::var_os("JUANOLA_ASR_MODEL")
            .map(PathBuf::from)
            .unwrap_or_else(|| models.join("stt_streaming/qwen3-asr-1.7b-4bit-mlx")),
        vad_dir: models.join("vad/silero-vad-mlx"),
        language,
        // `JUANOLA_ASR_CONTEXT` = archivo de texto para el prompt (nombres, términos).
        context: std::env::var_os("JUANOLA_ASR_CONTEXT").map(std::fs::read_to_string).transpose()?,
    })?;
    match worker.events().recv()? {
        WorkerEvent::Ready { load_ms } => println!("worker listo en {load_ms} ms"),
        other => anyhow::bail!("esperaba ready, llegó {other:?}"),
    }

    // Enviar en bloques de 20 ms al ritmo real.
    let started = Instant::now();
    const BLOCK: usize = 320;
    let mut lags = Lags::default();
    let n = tracks.iter().map(|(_, t)| t.len()).max().unwrap_or(0);
    let mut i = 0;
    while i < n {
        let due = Duration::from_secs_f64(i as f64 / 16_000.0);
        if let Some(wait) = due.checked_sub(started.elapsed()) {
            std::thread::sleep(wait);
        }
        for (channel, t) in &tracks {
            if i < t.len() {
                worker.send(*channel, &t[i..(i + BLOCK).min(t.len())])?;
            }
        }
        i += BLOCK;
        for ev in worker.events().try_iter() {
            report(&ev, started, &mut lags, quiet);
        }
    }
    // Un poco de silencio para cerrar la última frase y fin de la entrada.
    for (channel, _) in &tracks {
        worker.send(*channel, &vec![0.0; 16_000])?;
    }
    worker.finish();
    while let Ok(ev) = worker.events().recv_timeout(Duration::from_secs(60)) {
        report(&ev, started, &mut lags, quiet);
    }
    for (name, v) in [("parciales", &mut lags.partial), ("finales", &mut lags.fin)] {
        if v.is_empty() {
            continue;
        }
        v.sort_by(f64::total_cmp);
        let p = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
        println!("retraso {name} vs. fin del tramo: mediana {:.2}s, p90 {:.2}s, máx {:.2}s ({} resultados)", p(0.5), p(0.9), p(1.0), v.len());
    }
    if let Some(m) = lags.decode.iter().max() {
        let mean = lags.decode.iter().sum::<u64>() as f64 / lags.decode.len() as f64;
        println!("decodificación por resultado: media {mean:.0} ms, máx {m} ms");
    }
    Ok(())
}

#[derive(Default)]
struct Lags {
    partial: Vec<f64>,
    fin: Vec<f64>,
    decode: Vec<u64>,
}

fn report(ev: &WorkerEvent, started: Instant, lags: &mut Lags, quiet: bool) {
    match ev {
        WorkerEvent::Segment { id, start, end, text, is_final, decode_ms, .. } => {
            let lag = started.elapsed().as_secs_f64() - end;
            if *is_final { lags.fin.push(lag) } else { lags.partial.push(lag) }
            lags.decode.push(*decode_ms);
            if quiet {
                return;
            }
            println!(
                "{:>6.2}s  #{id} {} [{start:>5.1}-{end:>5.1}] retraso {lag:.2}s (decode {decode_ms} ms)  {text}",
                started.elapsed().as_secs_f64(),
                if *is_final { "FINAL  " } else { "parcial" },
            );
        }
        other => println!("{other:?}"),
    }
}
