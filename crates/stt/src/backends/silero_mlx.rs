//! Silero VAD sobre MLX (`mlxrs`), en streaming: una probabilidad de voz por
//! ventana de 512 muestras (32 ms a 16 kHz), con el estado LSTM arrastrado.

use std::path::Path;

use mlxrs::audio::vad::models::silero_vad::{SileroVadModel, SileroVadState};
use mlxrs::Array;

use crate::SttError;

pub const WINDOW: usize = 512;
const SAMPLE_RATE: u32 = 16_000;

fn err(e: impl std::fmt::Display) -> SttError {
    SttError::Backend(format!("silero: {e}"))
}

pub struct SileroMlx {
    model: SileroVadModel,
    state: Option<SileroVadState>,
}

impl SileroMlx {
    pub fn load(dir: &Path) -> Result<Self, SttError> {
        let path = dir.to_str().ok_or_else(|| err("ruta no UTF-8"))?;
        Ok(Self { model: SileroVadModel::load(path).map_err(err)?, state: None })
    }

    /// Probabilidad de voz de una ventana de exactamente `WINDOW` muestras.
    pub fn prob(&mut self, window: &[f32]) -> Result<f32, SttError> {
        debug_assert_eq!(window.len(), WINDOW);
        let chunk = Array::from_slice(window, &[WINDOW as i32]).map_err(err)?;
        let (mut p, state) = self.model.feed(&chunk, self.state.take(), SAMPLE_RATE).map_err(err)?;
        self.state = Some(state);
        let v: Vec<f32> = p.to_vec().map_err(err)?;
        v.first().copied().ok_or_else(|| err("salida vacía"))
    }

    pub fn reset(&mut self) {
        self.state = None;
    }
}
