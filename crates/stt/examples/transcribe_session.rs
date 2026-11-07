//! Transcribe una sesión grabada (mic.wav + system.wav, mono 16 kHz) con
//! `juanola-stt-worker` a máxima velocidad y guarda los segmentos finales.
//!
//! ```text
//! target/release/examples/transcribe_session <mic.wav> <system.wav> <salida.json> [idioma|auto]
//! ```

use std::path::PathBuf;
use std::time::{Duration, Instant};

use juanola_core_types::Channel;
use juanola_stt::worker::{SttWorker, WorkerConfig, WorkerEvent};

fn read(path: &str) -> anyhow::Result<Vec<f32>> {
    let mut r = hound::WavReader::open(path)?;
    anyhow::ensure!(r.spec().sample_rate == 16_000 && r.spec().channels == 1, "{path}: se espera mono 16 kHz");
    Ok(r.samples::<i16>().map(|s| s.map(|v| v as f32 / 32768.0)).collect::<Result<_, _>>()?)
}

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(a.len() >= 3, "uso: transcribe_session <mic.wav> <system.wav> <salida.json> [idioma|auto]");
    let (mic, sys) = (read(&a[0])?, read(&a[1])?);
    let language = match a.get(3).map(String::as_str) {
        Some("auto") => None,
        Some(l) => Some(l.to_owned()),
        None => Some("Spanish".to_owned()),
    };
    let models = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join("Library/Application Support/dev.juanola.desktop/models");
    let program = std::env::current_exe()?.parent().and_then(|p| p.parent()).unwrap().join("juanola-stt-worker");
    let mut w = SttWorker::spawn(&WorkerConfig {
        program,
        model_dir: models.join("stt_streaming/qwen3-asr-0.6b-mlx"),
        vad_dir: models.join("vad/silero-vad-mlx"),
        language,
    })?;
    let WorkerEvent::Ready { .. } = w.events().recv()? else { anyhow::bail!("el worker no arrancó") };

    let t = Instant::now();
    const BLOCK: usize = 1600;
    let n = mic.len().max(sys.len());
    let mut finals = Vec::new();
    let mut i = 0;
    while i < n {
        for (ch, data) in [(Channel::Mic, &mic), (Channel::System, &sys)] {
            if i < data.len() {
                w.send(ch, &data[i..(i + BLOCK).min(data.len())])?;
            }
        }
        i += BLOCK;
        finals.extend(w.events().try_iter().filter_map(|e| e.as_transcript()).filter(|t| t.is_final));
    }
    w.finish();
    while let Ok(ev) = w.events().recv_timeout(Duration::from_secs(30)) {
        if let Some(t) = ev.as_transcript().filter(|t| t.is_final) {
            finals.push(t);
        }
    }
    finals.retain(|t| !t.text.trim().is_empty());
    finals.sort_by(|x, y| x.start.total_cmp(&y.start));
    std::fs::write(&a[2], serde_json::to_string_pretty(&finals)?)?;
    let audio = n as f64 / 16_000.0;
    println!(
        "{} segmentos de {:.1} min de audio en {:.1}s ({:.0}x tiempo real) → {}",
        finals.len(),
        audio / 60.0,
        t.elapsed().as_secs_f64(),
        audio / t.elapsed().as_secs_f64(),
        a[2]
    );
    Ok(())
}
