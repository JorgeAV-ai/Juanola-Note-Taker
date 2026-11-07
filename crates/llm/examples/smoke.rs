//! Prueba rápida de LFM3 con llama.cpp: texto libre y salida con JSON Schema.
//! `cargo run --release -p juanola-llm --features llamacpp --example smoke [gguf]`

#[cfg(feature = "llamacpp")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use juanola_llm::backends::llamacpp::LlamaCpp;
    use juanola_llm::{GenOptions, Message};
    let path = std::env::args().nth(1).map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::PathBuf::from(std::env::var("HOME").unwrap())
            .join("Library/Application Support/dev.juanola.desktop/models/llm/lfm3-1.7b-q4km/LFM3-1.7B-Q4_K_M.gguf")
    });
    let t = std::time::Instant::now();
    let llm = LlamaCpp::load(&path, 8192)?;
    println!("cargado en {:.2}s", t.elapsed().as_secs_f32());
    let m = |r: &str, c: &str| Message { role: r.into(), content: c.into() };

    let (text, st) = llm.complete_blocking(
        &[m("system", "Responde en español, breve."), m("user", "¿Para qué sirve un acta de reunión? Dos frases.")],
        &GenOptions { max_tokens: Some(200), ..Default::default() },
    )?;
    println!("--- libre ({st:?}, {:.0} tok/s)\n{text}", st.generated_tokens as f64 / (st.decode_ms as f64 / 1000.0));

    let schema = serde_json::json!({"type":"object","required":["tareas"],"properties":{"tareas":{"type":"array","items":{
        "type":"object","required":["quien","que"],"properties":{"quien":{"type":"string"},"que":{"type":"string"}}}}}});
    let (text, st) = llm.complete_blocking(
        &[m("user", "Extrae las tareas: «Laura prepara las diapositivas para el jueves y Pedro se encarga de la demo.»")],
        &GenOptions { max_tokens: Some(300), json_schema: Some(schema), ..Default::default() },
    )?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    println!("--- json ({st:?})\n{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

#[cfg(not(feature = "llamacpp"))]
fn main() {}
