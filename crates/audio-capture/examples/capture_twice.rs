//! Diagnóstico: varias sesiones de captura seguidas en el mismo proceso, como
//! hace la app (Start → Stop → Start…). Imprime segundos de audio y pico por canal.
//! `capture_twice [sesiones] [segundos]`

use std::time::{Duration, Instant};

use juanola_audio_capture::{AudioCapture, CaptureConfig, CpalCapture};
use juanola_core_types::Channel;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let a: Vec<u64> = std::env::args().skip(1).filter_map(|s| s.parse().ok()).collect();
    let (sessions, secs) = (a.first().copied().unwrap_or(3), a.get(1).copied().unwrap_or(6));
    for n in 1..=sessions {
        let mut cap = CpalCapture::new();
        let mut rx =
            cap.start(CaptureConfig { mic_device: None, capture_mic: true, capture_system_audio: true, echo_cancellation: false })?;
        let t = Instant::now();
        let (mut mic_n, mut sys_n, mut mic_p, mut sys_p) = (0usize, 0usize, 0f32, 0f32);
        while t.elapsed() < Duration::from_secs(secs) {
            match rx.try_recv() {
                Ok(f) => {
                    let p = f.samples.iter().fold(0f32, |m, s| m.max(s.abs()));
                    match f.channel {
                        Channel::Mic => (mic_n += f.samples.len(), mic_p = mic_p.max(p)),
                        Channel::System => (sys_n += f.samples.len(), sys_p = sys_p.max(p)),
                    };
                }
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
        }
        cap.stop()?;
        drop(cap);
        println!(
            "SESIÓN {n}: mic {:.1}s pico {:.4} | sistema {:.1}s pico {:.4}",
            mic_n as f32 / 16000.0,
            mic_p,
            sys_n as f32 / 16000.0,
            sys_p
        );
        std::thread::sleep(Duration::from_millis(800));
    }
    Ok(())
}
