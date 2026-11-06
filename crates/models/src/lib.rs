//! Catálogo y gestor de modelos: `models.toml` por slot, descarga desde
//! Hugging Face con progreso, verificación SHA-256 y selección de backend GPU.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub slots: BTreeMap<String, Slot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slot {
    pub description: String,
    pub runtime: String, // sherpa-onnx | whisper.cpp | llama.cpp | fastembed | speakrs
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub name: String,
    pub repo: String,
    #[serde(default)]
    pub files: Vec<String>,
    pub sha256: Option<String>,
    pub size_mb: Option<u64>,
    #[serde(default)]
    pub backends: Vec<String>, // metal, coreml, cuda, vulkan, cpu
    pub license: Option<String>,
}

impl Catalog {
    pub fn parse(toml_src: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(toml_src)?)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn repo_catalog_parses() {
        let src = include_str!("../../../models.toml");
        let c = super::Catalog::parse(src).expect("models.toml válido");
        assert!(c.slots.contains_key("stt_streaming"));
    }
}
