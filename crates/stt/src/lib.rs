//! Reconocimiento de voz. Trait único con dos modos: streaming (transcript en
//! vivo por canal) y batch (re-transcripción de calidad al terminar).

use juanola_audio_capture::AudioFrame;
use juanola_core_types::Channel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub channel: Channel,
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub is_final: bool,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct SttOptions {
    /// `None` = multi-idioma / auto-detección.
    pub language: Option<String>,
    /// Personal + workspace jargon (hotwords o prompt inicial según backend).
    pub hotwords: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("modelo no cargado para el slot {0}")]
    ModelNotLoaded(&'static str),
    #[error("{0}")]
    Backend(String),
}

/// Reconocedor en streaming: una instancia por canal (mic / system).
pub trait StreamingTranscriber: Send {
    fn push(&mut self, frame: &AudioFrame) -> Result<Vec<Transcript>, SttError>;
    fn flush(&mut self) -> Result<Vec<Transcript>, SttError>;
}

/// Reconocedor batch sobre audio completo.
#[async_trait::async_trait]
pub trait BatchTranscriber: Send + Sync {
    async fn transcribe(
        &self,
        samples: &[f32],
        channel: Channel,
        opts: &SttOptions,
    ) -> Result<Vec<Transcript>, SttError>;
}

/// Detector de actividad de voz por frame (Silero / TEN vía sherpa-onnx).
pub trait Vad: Send {
    fn is_speech(&mut self, samples: &[f32]) -> bool;
}

pub mod backends;
pub mod context;
pub mod crosscheck;
pub mod language_lock;
pub mod segmenter;
pub mod worker;
