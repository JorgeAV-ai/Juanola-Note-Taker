//! Modelos que necesita la app: cuáles tocan en esta máquina (según
//! `models.toml`), si están instalados y su descarga desde Hugging Face.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use juanola_models::install::{self, APP_SLOTS, ModelStatus, Planned};
use juanola_models::{Catalog, Machine, Platform};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Mensaje común cuando se intenta grabar o generar notas sin los modelos.
pub const MISSING: &str = "Faltan modelos por descargar.";

pub struct Models {
    catalog: Catalog,
    machine: Machine,
    root: PathBuf,
    installing: AtomicBool,
    cancel: Arc<AtomicBool>,
}

#[derive(Clone, Serialize)]
struct Done {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Models {
    pub fn new(data_dir: &Path) -> Self {
        let catalog = Catalog::parse(include_str!("../../../../models.toml")).expect("models.toml válido");
        Self {
            catalog,
            machine: Machine { platform: Platform::current(), ram_gb: ram_gb() },
            root: data_dir.join("models"),
            installing: AtomicBool::new(false),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    fn plan(&self) -> Result<Vec<Planned<'_>>, String> {
        self.catalog.plan(APP_SLOTS, &self.machine, &self.root).map_err(|e| e.to_string())
    }

    pub fn status(&self) -> Result<Vec<ModelStatus>, String> {
        Ok(self.plan()?.iter().map(Planned::status).collect())
    }

    /// `Err(MISSING)` si falta alguno de `slots`.
    pub fn require(&self, slots: &[&str]) -> Result<(), String> {
        // Un slot fuera de APP_SLOTS también puede faltar (p. ej. diarization):
        // hay que planearlo para no darlo por instalado.
        let mut all: Vec<&str> = APP_SLOTS.to_vec();
        for s in slots {
            if !all.contains(s) {
                all.push(s);
            }
        }
        let missing = self
            .catalog
            .plan(&all, &self.machine, &self.root)
            .map_err(|e| e.to_string())?
            .iter()
            .any(|p| slots.contains(&p.slot.as_str()) && !p.status().installed);
        if missing { Err(MISSING.into()) } else { Ok(()) }
    }

    /// Carpeta del modelo de un slot.
    pub fn dir(&self, slot: &str) -> PathBuf {
        self.plan().ok().and_then(|p| p.into_iter().find(|p| p.slot == slot).map(|p| p.dir)).unwrap_or_else(|| self.root.join(slot))
    }

    /// El fichero principal del modelo de un slot (el GGUF del LLM).
    pub fn main_file(&self, slot: &str) -> PathBuf {
        let file = self.catalog.pick(slot, &self.machine).and_then(|e| e.files.first().cloned()).unwrap_or_default();
        self.dir(slot).join(file)
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Descarga en segundo plano lo que falte. Emite `models://progress` y, al
    /// acabar, `models://done`. Si ya hay una descarga en marcha no hace nada.
    pub fn start_install(self: &Arc<Self>, app: AppHandle) {
        if self.installing.swap(true, Ordering::SeqCst) {
            return;
        }
        self.cancel.store(false, Ordering::Relaxed);
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            // Si `install_all` paniquea, `installing` no debe quedarse pegado.
            struct Reset<'a>(&'a AtomicBool);
            impl Drop for Reset<'_> {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::SeqCst);
                }
            }
            let _reset = Reset(&me.installing);
            let result = me.install_all(&app).await;
            if let Err(e) = &result {
                tracing::warn!(error = %e, "descarga de modelos");
            }
            let _ = app.emit("models://done", Done { ok: result.is_ok(), error: result.err() });
        });
    }

    pub fn installing(&self) -> bool {
        self.installing.load(Ordering::SeqCst)
    }

    async fn install_all(&self, app: &AppHandle) -> Result<(), String> {
        let mut failed = Vec::new();
        for planned in self.plan()? {
            // Sin esta comprobación, cancelar solo se nota dentro de una
            // descarga: entre modelos se seguiría tirando.
            if self.cancel.load(Ordering::Relaxed) {
                return Err("descarga cancelada".into());
            }
            if planned.status().installed {
                continue;
            }
            tracing::info!(slot = planned.slot, repo = planned.entry.repo, "descargando modelo");
            // Como mucho ~10 eventos por segundo, más el último.
            let mut last = Instant::now() - Duration::from_secs(1);
            if let Err(e) = install::install(&planned, &self.cancel, |p| {
                if p.done == p.total || last.elapsed() >= Duration::from_millis(100) {
                    last = Instant::now();
                    let _ = app.emit("models://progress", &p);
                }
            })
            .await
            {
                // Un modelo que no se puede instalar (p. ej. paquete no HF aún
                // no implementado) no impide descargar el resto.
                if self.cancel.load(Ordering::Relaxed) {
                    return Err(e.to_string());
                }
                tracing::warn!(slot = planned.slot, error = %e, "no se pudo instalar un modelo");
                failed.push(format!("{}: {e}", planned.slot));
            }
        }
        if failed.is_empty() { Ok(()) } else { Err(failed.join("; ")) }
    }
}

/// RAM física en GB (para `min_ram_gb` del catálogo).
fn ram_gb() -> u64 {
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output()
            && let Ok(bytes) = String::from_utf8_lossy(&out.stdout).trim().parse::<u64>()
        {
            return bytes >> 30;
        }
    }
    8
}
