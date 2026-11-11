use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use juanola_llm::backends::llamacpp::LlamaCpp;

use crate::detect::{Detection, DetectionSettings};
use crate::models::Models;
use crate::recording::{Active, Paths};

pub struct AppState {
    pub db: juanola_db::Db,
    pub data_dir: PathBuf,
    /// Grabación en curso (una a la vez).
    pub recording: Mutex<Option<Active>>,
    /// `true` mientras `stop_recording` espera a que el hilo de grabación
    /// termine: el slot ya está libre pero no puede empezar otra sesión.
    pub stopping: AtomicBool,
    /// LFM3 se carga la primera vez que se hace un enhance y se queda en memoria
    /// hasta cerrar la app (ver `unload_llm`).
    pub llm: tokio::sync::Mutex<Option<Arc<LlamaCpp>>>,
    /// Catálogo, estado y descarga de los modelos.
    pub models: Arc<Models>,
    /// Detección de reuniones, avisos y caché del calendario.
    pub detection: Detection,
}

impl AppState {
    pub fn new(db: juanola_db::Db, data_dir: PathBuf, detection: DetectionSettings) -> Self {
        let models = Arc::new(Models::new(&data_dir));
        Self {
            db,
            data_dir,
            recording: Mutex::new(None),
            stopping: AtomicBool::new(false),
            llm: tokio::sync::Mutex::new(None),
            models,
            detection: Detection::new(detection),
        }
    }

    /// Libera el modelo antes de salir. Si sigue cargado cuando el proceso
    /// termina, los destructores de ggml-metal encuentran sus buffers de GPU
    /// aún vivos y abortan (`GGML_ASSERT([rsets->data count] == 0)`). Si hay
    /// un enhance en marcha, ese hilo aún tiene el modelo y no se puede.
    pub fn unload_llm(&self) {
        match self.llm.try_lock() {
            Ok(mut slot) => {
                if let Some(model) = slot.take() {
                    match Arc::try_unwrap(model) {
                        Ok(m) => {
                            drop(m);
                            tracing::info!("modelo LLM liberado al salir");
                        }
                        Err(_) => tracing::warn!("el modelo LLM sigue en uso al salir"),
                    }
                }
            }
            Err(_) => tracing::warn!("el modelo LLM se está cargando al salir"),
        }
    }

    pub fn paths(&self) -> Paths {
        Paths { stt_dir: self.models.dir("stt_streaming"), vad_dir: self.models.dir("vad") }
    }

    pub fn gguf(&self) -> PathBuf {
        self.models.main_file("llm_main")
    }
}
