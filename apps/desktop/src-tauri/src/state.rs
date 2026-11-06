use std::path::PathBuf;

#[allow(dead_code)] // `db` se usa desde los repos a partir de la fase 1
pub struct AppState {
    pub db: granola_db::Db,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new(db: granola_db::Db, data_dir: PathBuf) -> Self {
        Self { db, data_dir }
    }
}
