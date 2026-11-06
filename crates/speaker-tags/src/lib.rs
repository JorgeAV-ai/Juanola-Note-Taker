//! Speaker tags: lee nombre de participante e indicador "hablando" de Zoom
//! Workplace, Microsoft Teams (escritorio) y Google Meet (Chrome) vía
//! Accesibilidad. Emite intervalos (nombre, t0, t1) que `diarize` fusiona.

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveSpeaker {
    pub name: String,
    pub since: f64,
}

pub trait SpeakerTagSource: Send {
    fn poll(&mut self) -> Vec<ActiveSpeaker>;
}
