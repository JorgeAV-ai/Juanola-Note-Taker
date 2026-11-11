//! Grabación opcional a disco (`audio/<note_id>/{mic,system}.flac`), política de
//! retención y snapshots de pantalla compartida. Por defecto no se guarda audio; aquí es
//! configurable y permite reproducir y re-transcribir.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retention {
    Off,
    OneDay,
    OneWeek,
    OneMonth,
    ThreeMonths,
    SixMonths,
    OneYear,
}

pub struct AudioStore {
    pub root: PathBuf,
    pub keep_audio: bool,
    pub retention: Retention,
}
