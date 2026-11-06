//! Diarización acústica y fusión con speaker tags.
//! Prioridad: nombres por Accesibilidad > diarización > "Me"/"Them".

#[derive(Debug, Clone)]
pub struct SpeakerTurn {
    pub speaker: u32,
    pub start: f64,
    pub end: f64,
}

#[async_trait::async_trait]
pub trait Diarizer: Send + Sync {
    async fn diarize(&self, samples_16k: &[f32]) -> anyhow::Result<Vec<SpeakerTurn>>;
}

pub trait SpeakerEmbedder: Send + Sync {
    fn embed(&self, samples_16k: &[f32]) -> anyhow::Result<Vec<f32>>;
}
