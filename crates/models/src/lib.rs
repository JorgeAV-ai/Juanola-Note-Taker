//! Catálogo y gestor de modelos: `models.toml` por slot, descarga desde
//! Hugging Face con progreso, verificación SHA-256 y selección de backend
//! según plataforma, GPU y RAM. Ver docs/modelos-mvp.md.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub slots: BTreeMap<String, Slot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slot {
    pub description: String,
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub name: String,
    /// mlx | sherpa-onnx | llama.cpp | speakrs | fastembed | fluidaudio
    pub runtime: String,
    /// mlx | onnx | gguf | coreml
    pub format: String,
    /// Repo de Hugging Face (`org/nombre`) o URL directa a un archivo.
    pub repo: String,
    #[serde(default)]
    pub files: Vec<String>,
    pub sha256: Option<String>,
    pub size_mb: Option<u64>,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub backends: Vec<String>,
    pub min_ram_gb: Option<u64>,
    pub context: Option<u64>,
    pub languages: Option<u32>,
    pub wer_en: Option<f32>,
    pub dimension: Option<u32>,
    pub notes: Option<String>,
    /// `false` = el runtime Rust aún no ejecuta esta entrada; el selector la salta.
    #[serde(default = "default_true")]
    pub ready: bool,
}

fn default_true() -> bool {
    true
}

/// Plataforma en la que corre la app, en el mismo vocabulario que `models.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacosArm64,
    MacosX64,
    WindowsX64,
    LinuxX64,
}

impl Platform {
    pub fn current() -> Self {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") => Self::MacosArm64,
            ("macos", _) => Self::MacosX64,
            ("windows", _) => Self::WindowsX64,
            _ => Self::LinuxX64,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::MacosArm64 => "macos-arm64",
            Self::MacosX64 => "macos-x64",
            Self::WindowsX64 => "windows-x64",
            Self::LinuxX64 => "linux-x64",
        }
    }

    /// Formatos nativos en orden de preferencia.
    pub fn preferred_formats(self) -> &'static [&'static str] {
        match self {
            Self::MacosArm64 => &["mlx", "coreml", "gguf", "onnx"],
            Self::MacosX64 => &["gguf", "onnx"],
            Self::WindowsX64 | Self::LinuxX64 => &["gguf", "onnx"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Machine {
    pub platform: Platform,
    pub ram_gb: u64,
}

impl Catalog {
    pub fn parse(toml_src: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(toml_src)?)
    }

    /// Elige, para un slot, la entrada por defecto en el formato más nativo
    /// disponible para la máquina. Devuelve `None` si el slot no tiene modelos
    /// aptos (por ejemplo, un slot reservado fuera del MVP).
    pub fn pick(&self, slot: &str, machine: &Machine) -> Option<&ModelEntry> {
        let s = self.slots.get(slot)?;
        let name = s.default.as_deref()?;
        let plat = machine.platform.key();
        let candidates: Vec<&ModelEntry> = s
            .models
            .iter()
            .filter(|m| m.name == name)
            .filter(|m| m.ready)
            .filter(|m| m.platforms.iter().any(|p| p == plat))
            .filter(|m| m.min_ram_gb.map_or(true, |min| machine.ram_gb >= min))
            .collect();
        machine
            .platform
            .preferred_formats()
            .iter()
            .find_map(|fmt| candidates.iter().find(|m| m.format == *fmt).copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        Catalog::parse(include_str!("../../../models.toml")).expect("models.toml válido")
    }

    #[test]
    fn repo_catalog_parses() {
        let c = catalog();
        assert!(c.slots.contains_key("stt_streaming"));
        assert!(c.slots.contains_key("llm_main"));
    }

    #[test]
    fn mac_arm_prefers_native_ready_formats() {
        let c = catalog();
        let m = Machine { platform: Platform::MacosArm64, ram_gb: 16 };
        assert_eq!(c.pick("stt_streaming", &m).unwrap().format, "mlx");
        // LFM3 en MLX está marcado ready = false hasta que mlxrs lo soporte: cae a GGUF (Metal).
        let llm = c.pick("llm_main", &m).unwrap();
        assert_eq!(llm.name, "lfm3-1.7b");
        assert_eq!(llm.format, "gguf");
        assert_eq!(c.pick("diarization", &m).unwrap().format, "coreml");
        let emb = c.pick("text_embedding", &m).unwrap();
        assert_eq!(emb.name, "embeddinggemma-2-text");
        assert_eq!(emb.format, "onnx");
    }

    #[test]
    fn windows_prefers_gguf_and_onnx() {
        let c = catalog();
        let m = Machine { platform: Platform::WindowsX64, ram_gb: 16 };
        assert_eq!(c.pick("stt_streaming", &m).unwrap().format, "onnx");
        let llm = c.pick("llm_main", &m).unwrap();
        assert_eq!(llm.name, "lfm3-1.7b");
        assert_eq!(llm.format, "gguf");
        assert_eq!(c.pick("diarization", &m).unwrap().format, "onnx");
        assert_eq!(c.pick("text_embedding", &m).unwrap().format, "onnx");
    }

    #[test]
    fn not_ready_entries_are_skipped() {
        let c = catalog();
        let slot = &c.slots["llm_main"];
        assert!(slot.models.iter().any(|m| m.format == "mlx" && !m.ready));
    }

    #[test]
    fn empty_slots_pick_nothing() {
        let c = catalog();
        let m = Machine { platform: Platform::WindowsX64, ram_gb: 16 };
        assert!(c.pick("reranker", &m).is_none());
    }
}
