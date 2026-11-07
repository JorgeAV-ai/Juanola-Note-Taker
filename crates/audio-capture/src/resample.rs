//! Resampler mono en streaming hacia `TARGET_SAMPLE_RATE`.
//!
//! Envuelve `rubato::Fft` (ratio fijo: la frecuencia del dispositivo no cambia
//! durante un stream de cpal) con un búfer de entrada para aceptar bloques de
//! cualquier tamaño, que es lo que entregan los callbacks de audio.

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

use crate::TARGET_SAMPLE_RATE;

const CHUNK: usize = 1024;

pub struct MonoResampler {
    inner: Option<Fft<f32>>,
    pending: Vec<f32>,
    out: Vec<f32>,
}

impl MonoResampler {
    pub fn new(input_rate: u32) -> Result<Self, String> {
        let inner = if input_rate == TARGET_SAMPLE_RATE {
            None
        } else {
            Some(
                Fft::<f32>::new(
                    input_rate as usize,
                    TARGET_SAMPLE_RATE as usize,
                    CHUNK,
                    1,
                    FixedSync::Input,
                )
                .map_err(|e| e.to_string())?,
            )
        };
        let out_cap = inner.as_ref().map_or(0, |r| r.output_frames_max());
        Ok(Self { inner, pending: Vec::with_capacity(CHUNK * 2), out: vec![0.0; out_cap] })
    }

    /// Añade muestras a la frecuencia de entrada y devuelve las que ya se han
    /// podido convertir (puede ser un vector vacío).
    pub fn push(&mut self, samples: &[f32]) -> Vec<f32> {
        let Some(r) = self.inner.as_mut() else {
            return samples.to_vec();
        };
        self.pending.extend_from_slice(samples);
        let mut result = Vec::new();
        let mut consumed = 0;
        loop {
            let need = r.input_frames_next();
            if self.pending.len() - consumed < need {
                break;
            }
            let input = &self.pending[consumed..consumed + need];
            let in_adapter = InterleavedSlice::new(input, 1, need).expect("input adapter");
            let out_len = self.out.len();
            let mut out_adapter =
                InterleavedSlice::new_mut(&mut self.out[..], 1, out_len).expect("output adapter");
            match r.process_into_buffer(&in_adapter, &mut out_adapter, None) {
                Ok((n_in, n_out)) => {
                    consumed += n_in;
                    result.extend_from_slice(&self.out[..n_out]);
                }
                Err(e) => {
                    tracing::warn!("resample falló: {e}");
                    consumed += need;
                }
            }
        }
        self.pending.drain(..consumed);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_at_target_rate() {
        let mut r = MonoResampler::new(TARGET_SAMPLE_RATE).unwrap();
        assert_eq!(r.push(&[0.5; 100]).len(), 100);
    }

    #[test]
    fn downsample_48k_keeps_ratio() {
        let mut r = MonoResampler::new(48_000).unwrap();
        let mut total = 0;
        // 1 s de audio en bloques irregulares, como los callbacks reales.
        for block in [480usize, 512, 333, 1024].iter().cycle().take(200) {
            total += r.push(&vec![0.1; *block]).len();
        }
        let input: usize = [480usize, 512, 333, 1024].iter().cycle().take(200).sum();
        let expected = input / 3;
        // Lo pendiente en el búfer y el retardo del filtro caben en un chunk.
        assert!(expected - total <= CHUNK, "esperado ~{expected}, salió {total}");
    }
}
