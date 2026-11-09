//! Transcribe un WAV mono a 16 kHz con Qwen3-ASR sobre MLX.
//!
//! ```text
//! cargo run --release -p juanola-stt --features mlx --example transcribe_wav -- <wav> [idioma|auto] [carpeta_modelo]
//! ```
//!
//! Por defecto usa el modelo en la carpeta de datos de la app y el idioma "Spanish".

#[cfg(all(feature = "mlx", target_os = "macos"))]
fn main() -> anyhow::Result<()> {
    use std::path::PathBuf;
    use std::time::Instant;

    use juanola_stt::backends::qwen3_mlx::Qwen3AsrMlx;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let wav = args.first().ok_or_else(|| anyhow::anyhow!("uso: transcribe_wav <wav> [idioma|auto] [modelo]"))?;
    let language = match args.get(1).map(String::as_str) {
        Some("auto") => None,
        Some(l) => Some(l.to_owned()),
        None => Some("Spanish".to_owned()),
    };
    let model_dir = args.get(2).map(PathBuf::from).unwrap_or_else(|| {
        dirs_home().join("Library/Application Support/dev.juanola.desktop/models/stt_streaming/qwen3-asr-1.7b-4bit-mlx")
    });

    let mut reader = hound::WavReader::open(wav)?;
    let spec = reader.spec();
    anyhow::ensure!(spec.sample_rate == 16_000 && spec.channels == 1, "se espera WAV mono a 16 kHz, es {spec:?}");
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => reader.samples::<i16>().map(|s| s.map(|v| v as f32 / 32768.0)).collect::<Result<_, _>>()?,
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
    };
    let secs = samples.len() as f32 / 16_000.0;

    let t = Instant::now();
    let model = Qwen3AsrMlx::load(&model_dir)?;
    println!("modelo cargado en {:.2}s", t.elapsed().as_secs_f32());

    for run in 0..2 {
        let t = Instant::now();
        let out = model.transcribe(&samples, language.as_deref())?;
        let el = t.elapsed().as_secs_f32();
        println!(
            "[{}] {:.2}s para {:.1}s de audio (RTF {:.3}), {} tokens, idioma {:?}\n{}",
            if run == 0 { "frío" } else { "caliente" },
            el,
            secs,
            el / secs,
            out.generated_tokens,
            out.language,
            out.text
        );
    }
    Ok(())
}

#[cfg(all(feature = "mlx", target_os = "macos"))]
fn dirs_home() -> std::path::PathBuf {
    std::env::var_os("HOME").map(Into::into).unwrap_or_default()
}

#[cfg(not(all(feature = "mlx", target_os = "macos")))]
fn main() {
    eprintln!("compila con --features mlx en macOS");
}
