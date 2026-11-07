//! Implementaciones concretas, gated por features de Cargo.

#[cfg(all(feature = "mlx", target_os = "macos"))]
pub mod qwen3_mlx;
#[cfg(all(feature = "mlx", target_os = "macos"))]
pub mod silero_mlx;
#[cfg(feature = "sherpa")]
pub mod sherpa {}
#[cfg(feature = "whisper")]
pub mod whisper {}
