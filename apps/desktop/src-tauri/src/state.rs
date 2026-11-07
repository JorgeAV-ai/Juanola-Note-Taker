use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use juanola_llm::backends::llamacpp::LlamaCpp;
use tokio::sync::OnceCell;

use crate::recording::{Active, Paths};

pub struct AppState {
    pub db: juanola_db::Db,
    pub data_dir: PathBuf,
    /// Grabación en curso (una a la vez).
    pub recording: Mutex<Option<Active>>,
    /// LFM3 se carga la primera vez que se hace un enhance y se queda en memoria.
    pub llm: OnceCell<Arc<LlamaCpp>>,
}

impl AppState {
    pub fn new(db: juanola_db::Db, data_dir: PathBuf) -> Self {
        Self { db, data_dir, recording: Mutex::new(None), llm: OnceCell::new() }
    }

    pub fn paths(&self) -> Paths {
        Paths { data_dir: self.data_dir.clone() }
    }

    pub fn gguf(&self) -> PathBuf {
        self.data_dir.join("models/llm/lfm3-1.7b-q4km/LFM3-1.7B-Q4_K_M.gguf")
    }
}
