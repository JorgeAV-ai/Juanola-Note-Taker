//! Prueba de prompts: `prompt_lab <system.txt> <user.txt> [max_tokens]` imprime la respuesta de LFM3.
#[cfg(feature = "llamacpp")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use juanola_llm::backends::llamacpp::LlamaCpp;
    use juanola_llm::{GenOptions, Message};
    let a: Vec<String> = std::env::args().skip(1).collect();
    let gguf = std::path::PathBuf::from(std::env::var("HOME")?)
        .join("Library/Application Support/dev.juanola.desktop/models/llm/lfm3-1.7b-q8/LFM3-1.7B-Q8_0.gguf");
    // Ajustes para medir rendimiento: JUANOLA_LLM_UBATCH, JUANOLA_LLM_FA=0|1.
    let mut llm = LlamaCpp::load(&gguf, 32_768)?;
    if let Some(u) = std::env::var("JUANOLA_LLM_UBATCH").ok().and_then(|v| v.parse().ok()) {
        llm = llm.with_ubatch(u);
    }
    if let Ok(fa) = std::env::var("JUANOLA_LLM_FA") {
        llm = llm.with_flash_attention(Some(fa == "1"));
    }
    let sys = std::fs::read_to_string(&a[0])?;
    let user = std::fs::read_to_string(&a[1])?;
    let mut msgs = Vec::new();
    if !sys.trim().is_empty() {
        msgs.push(Message { role: "system".into(), content: sys });
    }
    msgs.push(Message { role: "user".into(), content: user });
    let max = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let (text, st) = llm.complete_blocking(&msgs, &GenOptions { max_tokens: Some(max), ..Default::default() })?;
    eprintln!("{st:?}");
    println!("{text}");
    Ok(())
}
#[cfg(not(feature = "llamacpp"))]
fn main() {}
