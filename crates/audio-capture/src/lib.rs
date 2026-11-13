//! Captura dual (micrófono + audio del sistema), resample a 16 kHz mono,
//! medidores de nivel para las barras animadas de la píldora.
//!
//! Por plataforma (ver docs/plan.md §5):
//! - macOS 14.6+: Core Audio process tap vía cpal (sin permiso de grabación de pantalla).
//! - macOS 13-14.5: ScreenCaptureKit solo audio (pendiente).
//! - Windows: WASAPI loopback.
//! - Linux: fuente `.monitor` de PipeWire/Pulse.

use juanola_core_types::Channel;
use tokio::sync::broadcast;

mod cpal_backend;
mod permissions;
mod resample;

pub use cpal_backend::CpalCapture;
pub use permissions::{Permission, mic_permission, request_mic_permission};
pub use resample::MonoResampler;

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
    /// Abrir el micrófono. Siempre en la app; sin él solo se graba el sistema
    /// (pruebas automáticas que no pueden aceptar el permiso de micrófono).
    pub capture_mic: bool,
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
