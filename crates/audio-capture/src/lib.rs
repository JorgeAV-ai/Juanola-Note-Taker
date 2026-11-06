//! Captura dual (micrófono + audio del sistema), resample a 16 kHz mono,
//! medidores de nivel para las barras animadas de la píldora.
//!
//! Por plataforma (ver docs/plan.md §5):
//! - macOS 14.2+: Core Audio process tap (sin permiso de grabación de pantalla).
//! - macOS 13-14.1: ScreenCaptureKit solo audio.
//! - Windows: WASAPI loopback.
//! - Linux: fuente `.monitor` de PipeWire/Pulse.

use granola_core_types::Channel;
use tokio::sync::broadcast;

/// Frecuencia a la que se entregan los frames a los modelos.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub channel: Channel,
    /// PCM f32 mono a `TARGET_SAMPLE_RATE`.
    pub samples: Vec<f32>,
    /// Segundos desde el inicio de la sesión.
    pub timestamp: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_bluetooth: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CaptureConfig {
    /// `None` = "Auto": seguir al micrófono que usa la app de reunión.
    pub mic_device: Option<String>,
    pub capture_system_audio: bool,
    pub echo_cancellation: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("permiso denegado: {0}")]
    PermissionDenied(&'static str),
    #[error("captura de audio del sistema no disponible en esta versión del SO")]
    SystemAudioUnavailable,
    #[error("dispositivo no encontrado: {0}")]
    DeviceNotFound(String),
    #[error("{0}")]
    Backend(String),
}

pub trait AudioCapture: Send {
    fn list_input_devices(&self) -> Result<Vec<AudioDevice>, CaptureError>;
    fn start(&mut self, cfg: CaptureConfig) -> Result<broadcast::Receiver<AudioFrame>, CaptureError>;
    fn pause(&mut self) -> Result<(), CaptureError>;
    fn resume(&mut self) -> Result<(), CaptureError>;
    fn stop(&mut self) -> Result<(), CaptureError>;
}

/// Nivel RMS por canal para los VU meters de la UI (20 Hz).
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct Levels {
    pub mic: f32,
    pub system: f32,
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rms_of_silence_is_zero() {
        assert_eq!(rms(&[0.0; 160]), 0.0);
    }
}
