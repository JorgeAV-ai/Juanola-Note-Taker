//! Proceso hijo de transcripción (`juanola-stt-worker`) y su cliente.
//!
//! El STT corre fuera del proceso de la app porque los `Array` de MLX no son
//! `Send` y un fallo de Metal aborta el proceso entero. Protocolo:
//!
//! - **stdin** (binario): tramas `[canal: u8][n: u32 LE][n × f32 LE]` con PCM
//!   mono a 16 kHz. Canal 0 = micrófono, 1 = sistema. El tiempo de cada canal
//!   es su número de muestras recibidas (la captura ya rellena huecos).
//!   Cerrar stdin = fin de la sesión: se emiten los finales pendientes.
//! - **stdout** (texto): un [`WorkerEvent`] JSON por línea.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;

use juanola_core_types::Channel;
use serde::{Deserialize, Serialize};

use crate::{SttError, Transcript};

pub const TARGET_SAMPLE_RATE: f64 = 16_000.0;

pub fn channel_byte(c: Channel) -> u8 {
    match c {
        Channel::Mic => 0,
        Channel::System => 1,
    }
}

pub fn channel_from_byte(b: u8) -> Option<Channel> {
    match b {
        0 => Some(Channel::Mic),
        1 => Some(Channel::System),
        _ => None,
    }
}

/// Escribe una trama de audio en el formato de stdin del worker.
pub fn write_frame(w: &mut impl Write, channel: Channel, samples: &[f32]) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(5 + samples.len() * 4);
    buf.push(channel_byte(channel));
    buf.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    for s in samples {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    w.write_all(&buf)
}

/// Lee una trama; `Ok(None)` al cerrarse la entrada.
pub fn read_frame(r: &mut impl Read) -> std::io::Result<Option<(Channel, Vec<f32>)>> {
    let mut head = [0u8; 5];
    match r.read_exact(&mut head) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let channel = channel_from_byte(head[0]).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "canal desconocido"))?;
    let n = u32::from_le_bytes([head[1], head[2], head[3], head[4]]) as usize;
    let mut bytes = vec![0u8; n * 4];
    r.read_exact(&mut bytes)?;
    let samples = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
    Ok(Some((channel, samples)))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WorkerEvent {
    Ready {
        load_ms: u64,
    },
    Segment {
        channel: Channel,
        /// Igual para los parciales y el final de una misma frase: la UI sustituye.
        id: u64,
        start: f64,
        end: f64,
        text: String,
        is_final: bool,
        language: Option<String>,
        /// Tiempo de decodificación de este tramo.
        decode_ms: u64,
        /// Probabilidad del token menos seguro del texto (ver
        /// `AsrOutput::token_probs`); sirve para decidir si se revisa la frase.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        confidence: Option<f32>,
        /// Probabilidad de cada token; solo en los finales.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        token_probs: Vec<f32>,
        /// Texto de cada token (mismo orden que `token_probs`); solo en los finales.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        token_texts: Vec<String>,
    },
    Error {
        message: String,
    },
}

impl WorkerEvent {
    pub fn as_transcript(&self) -> Option<Transcript> {
        match self {
            WorkerEvent::Segment { channel, start, end, text, is_final, confidence, .. } => Some(Transcript {
                channel: *channel,
                start: *start,
                end: *end,
                text: text.clone(),
                is_final: *is_final,
                confidence: *confidence,
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Ruta al binario `juanola-stt-worker`.
    pub program: PathBuf,
    pub model_dir: PathBuf,
    pub vad_dir: PathBuf,
    /// Nombre en inglés ("Spanish"); `None` = detección automática.
    pub language: Option<String>,
    /// Contexto para el prompt del modelo (nombres, términos); `None` = sin él.
    pub context: Option<String>,
}

/// Cliente: lanza el worker, le envía audio y recibe eventos.
pub struct SttWorker {
    child: Child,
    stdin: Option<std::io::BufWriter<ChildStdin>>,
    events: mpsc::Receiver<WorkerEvent>,
}

impl SttWorker {
    pub fn spawn(cfg: &WorkerConfig) -> Result<Self, SttError> {
        let mut cmd = Command::new(&cfg.program);
        cmd.arg("--model").arg(&cfg.model_dir).arg("--vad").arg(&cfg.vad_dir);
        cmd.arg("--language").arg(cfg.language.as_deref().unwrap_or("auto"));
        if let Some(c) = cfg.context.as_deref().filter(|c| !c.trim().is_empty()) {
            cmd.arg("--context").arg(c);
        }
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|e| SttError::Backend(format!("no se pudo lanzar {}: {e}", cfg.program.display())))?;
        let stdin = child.stdin.take().map(std::io::BufWriter::new);
        let stdout = child.stdout.take().ok_or_else(|| SttError::Backend("sin stdout".into()))?;
        let (tx, events) = mpsc::channel();
        std::thread::Builder::new()
            .name("stt-worker-reader".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    let ev = serde_json::from_str::<WorkerEvent>(&line)
                        .unwrap_or_else(|e| WorkerEvent::Error { message: format!("línea no válida del worker ({e}): {line}") });
                    if tx.send(ev).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| SttError::Backend(e.to_string()))?;
        Ok(Self { child, stdin, events })
    }

    pub fn send(&mut self, channel: Channel, samples: &[f32]) -> Result<(), SttError> {
        let w = self.stdin.as_mut().ok_or_else(|| SttError::Backend("worker cerrado".into()))?;
        write_frame(w, channel, samples).and_then(|_| w.flush()).map_err(|e| SttError::Backend(format!("el worker no acepta audio: {e}")))
    }

    /// Estado de salida si el proceso ya terminó.
    pub fn exit_status(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    pub fn events(&self) -> &mpsc::Receiver<WorkerEvent> {
        &self.events
    }

    /// Cierra stdin para que el worker emita los finales pendientes y termine.
    pub fn finish(&mut self) {
        self.stdin = None;
    }

    pub fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.finish();
        self.child.wait()
    }
}

impl Drop for SttWorker {
    fn drop(&mut self) {
        self.finish();
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, Channel::System, &[0.5, -0.25, 1.0]).unwrap();
        write_frame(&mut buf, Channel::Mic, &[]).unwrap();
        let mut r = std::io::Cursor::new(buf);
        let (c, s) = read_frame(&mut r).unwrap().unwrap();
        assert_eq!(c, Channel::System);
        assert_eq!(s, vec![0.5, -0.25, 1.0]);
        let (c, s) = read_frame(&mut r).unwrap().unwrap();
        assert_eq!((c, s.len()), (Channel::Mic, 0));
        assert!(read_frame(&mut r).unwrap().is_none());
    }

    #[test]
    fn event_json_shape() {
        let ev = WorkerEvent::Segment {
            channel: Channel::Mic,
            id: 3,
            start: 1.0,
            end: 2.5,
            text: "hola".into(),
            is_final: false,
            language: Some("Spanish".into()),
            decode_ms: 120,
            confidence: Some(0.42),
            token_probs: vec![],
            token_texts: vec![],
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.starts_with(r#"{"event":"segment","channel":"mic""#), "{s}");
        assert!(!s.contains("token_probs"), "{s}");
        let back: WorkerEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back.as_transcript().unwrap().confidence, Some(0.42));
    }
}
