//! Diagnóstico: dos sesiones captura → worker STT seguidas en el mismo proceso
//! (lo que hace la app con Start/Stop/Start). Imprime los finales de cada una.
use std::path::PathBuf;
use std::time::{Duration, Instant};

use juanola_audio_capture::{AudioCapture, CaptureConfig, CpalCapture};
use juanola_stt::worker::{SttWorker, WorkerConfig, WorkerEvent};

fn main() -> anyhow::Result<()> {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(12);
    let models = PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Library/Application Support/dev.juanola.desktop/models");
    let program = std::env::current_exe()?.parent().unwrap().join("juanola-stt-worker");
    for n in 1..=2 {
        let mut w = SttWorker::spawn(&WorkerConfig {
            program: program.clone(),
            model_dir: models.join("stt_streaming/qwen3-asr-1.7b-4bit-mlx"),
            vad_dir: models.join("vad/silero-vad-mlx"),
            language: Some("Spanish".into()),
            context: None,
        })?;
        let WorkerEvent::Ready { .. } = w.events().recv_timeout(Duration::from_secs(60))? else { anyhow::bail!("worker") };
        let mut cap = CpalCapture::new();
        let mut rx =
            cap.start(CaptureConfig { mic_device: None, capture_mic: true, capture_system_audio: true, echo_cancellation: false })?;
        let t = Instant::now();
        let (mut sent, mut finals) = (0usize, Vec::new());
        while t.elapsed() < Duration::from_secs(secs) {
            match rx.try_recv() {
                Ok(f) => {
                    sent += f.samples.len();
                    w.send(f.channel, &f.samples)?;
                }
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
            finals.extend(w.events().try_iter().filter_map(|e| e.as_transcript()).filter(|t| t.is_final));
        }
        cap.stop()?;
        w.finish();
        while let Ok(e) = w.events().recv_timeout(Duration::from_secs(15)) {
            finals.extend(e.as_transcript().filter(|t| t.is_final));
        }
        println!("SESIÓN {n}: {:.1}s de audio enviado, {} finales", sent as f32 / 16000.0, finals.len());
        for f in &finals {
            println!("   {:?} [{:.1}-{:.1}] {}", f.channel, f.start, f.end, f.text);
        }
        drop(w);
        std::thread::sleep(Duration::from_millis(800));
    }
    Ok(())
}
