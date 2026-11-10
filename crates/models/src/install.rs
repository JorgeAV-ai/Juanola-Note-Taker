//! Instalación de modelos desde Hugging Face: qué falta, descarga con progreso
//! y reanudación, y verificación SHA-256.
//!
//! Cada entrada se instala en `<raíz>/<dir>` (`dir` en `models.toml`, relativo
//! a la raíz) o, si no lo tiene, en `<raíz>/<slot>/<name>-<format>`. Un fichero a medio bajar se guarda como
//! `<fichero>.part` y se continúa con una petición `Range`. Los ficheros LFS se
//! comprueban con el SHA-256 que publica la API de Hugging Face; los pequeños
//! (no LFS) por tamaño.

use crate::{Catalog, Machine, ModelEntry};
use anyhow::{anyhow, bail, Context, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::AsyncWriteExt;

/// Slots que la app necesita hoy para grabar y generar notas.
pub const APP_SLOTS: &[&str] = &["vad", "stt_streaming", "llm_main"];

const HF: &str = "https://huggingface.co";

/// Un modelo elegido para esta máquina, con su carpeta de instalación.
#[derive(Debug, Clone)]
pub struct Planned<'a> {
    pub slot: String,
    pub entry: &'a ModelEntry,
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelStatus {
    pub slot: String,
    pub name: String,
    pub format: String,
    pub repo: String,
    pub size_mb: Option<u64>,
    pub dir: PathBuf,
    pub installed: bool,
    /// Bytes ya descargados de los ficheros que faltan (`.part` incluidos).
    pub partial_bytes: u64,
}

/// Progreso de una instalación, para la interfaz.
#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub slot: String,
    pub file: String,
    pub done: u64,
    pub total: u64,
}

impl Catalog {
    /// Los modelos de `slots` para esta máquina, con su carpeta bajo `root`.
    pub fn plan<'a>(&'a self, slots: &[&str], machine: &Machine, root: &Path) -> Result<Vec<Planned<'a>>> {
        slots
            .iter()
            .map(|slot| {
                let entry = self.pick(slot, machine).ok_or_else(|| anyhow!("no hay modelo para «{slot}» en esta máquina"))?;
                Ok(Planned { slot: slot.to_string(), entry, dir: root.join(entry.install_dir(slot)) })
            })
            .collect()
    }
}

impl ModelEntry {
    /// Carpeta de instalación relativa a la raíz de modelos.
    pub fn install_dir(&self, slot: &str) -> String {
        self.dir.clone().unwrap_or_else(|| format!("{slot}/{}-{}", self.name, self.format))
    }

    /// Ficheros que deben existir tras instalar: los del repo y los derivados.
    fn expected_files(&self) -> Vec<String> {
        let mut files = self.files.clone();
        if self.needs_tokenizer_json() {
            files.push("tokenizer.json".into());
        }
        files
    }

    /// Repos con el tokenizador de Qwen2 sin `tokenizer.json` (Qwen3-ASR MLX).
    fn needs_tokenizer_json(&self) -> bool {
        let has = |f: &str| self.files.iter().any(|x| x == f);
        has("vocab.json") && has("merges.txt") && !has("tokenizer.json")
    }
}

impl Planned<'_> {
    pub fn status(&self) -> ModelStatus {
        let mut installed = true;
        let mut partial_bytes = 0;
        for f in self.entry.expected_files() {
            let path = self.dir.join(&f);
            if !path.is_file() {
                installed = false;
                partial_bytes += std::fs::metadata(part_path(&path)).map(|m| m.len()).unwrap_or(0);
            }
        }
        ModelStatus {
            slot: self.slot.clone(),
            name: self.entry.name.clone(),
            format: self.entry.format.clone(),
            repo: self.entry.repo.clone(),
            size_mb: self.entry.size_mb,
            dir: self.dir.clone(),
            installed,
            partial_bytes,
        }
    }
}

#[derive(Debug, Deserialize)]
struct TreeEntry {
    path: String,
    size: u64,
    lfs: Option<Lfs>,
}

#[derive(Debug, Deserialize)]
struct Lfs {
    oid: String,
}

fn part_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}

/// Descarga lo que falte de `planned`. Llama a `on_progress` durante la
/// descarga y se detiene (dejando los `.part`) si `cancel` se activa.
pub async fn install(planned: &Planned<'_>, cancel: &AtomicBool, mut on_progress: impl FnMut(Progress)) -> Result<()> {
    let entry = planned.entry;
    if entry.repo.starts_with("http") {
        bail!("{}: la descarga de paquetes (no repos de Hugging Face) aún no está implementada", entry.name);
    }
    tokio::fs::create_dir_all(&planned.dir).await?;
    let client = reqwest::Client::builder().user_agent("juanola-model-installer").build()?;

    let missing: Vec<&String> = entry.files.iter().filter(|f| !planned.dir.join(f).is_file()).collect();
    if !missing.is_empty() {
        let tree: Vec<TreeEntry> = client
            .get(format!("{HF}/api/models/{}/tree/main", entry.repo))
            .send()
            .await?
            .error_for_status()
            .with_context(|| format!("no se pudo leer el repo {}", entry.repo))?
            .json()
            .await?;
        let mut jobs = Vec::new();
        for f in &missing {
            let t = tree.iter().find(|t| &t.path == *f).ok_or_else(|| anyhow!("{} no tiene {f}", entry.repo))?;
            // Si models.toml fija el SHA-256, manda sobre la API.
            let sha = match (&entry.sha256, entry.files.len()) {
                (Some(pinned), 1) => Some(pinned.clone()),
                _ => t.lfs.as_ref().map(|l| l.oid.clone()),
            };
            jobs.push((f.as_str(), t.size, sha));
        }
        let total: u64 = jobs.iter().map(|j| j.1).sum();
        let mut done_before = 0;
        for (file, size, sha) in jobs {
            let url = format!("{HF}/{}/resolve/main/{file}", entry.repo);
            let dest = planned.dir.join(file);
            download(&client, &url, &dest, size, sha.as_deref(), cancel, |n| {
                on_progress(Progress { slot: planned.slot.clone(), file: file.to_string(), done: done_before + n, total })
            })
            .await?;
            done_before += size;
        }
    }
    if entry.needs_tokenizer_json() {
        let dir = planned.dir.clone();
        tokio::task::spawn_blocking(move || crate::tokenizer::ensure_qwen2_tokenizer_json(&dir)).await??;
    }
    Ok(())
}

/// Descarga `url` en `dest` pasando por `dest.part`, continuando lo que haya.
pub async fn download(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    size: u64,
    sha256: Option<&str>,
    cancel: &AtomicBool,
    mut on_bytes: impl FnMut(u64),
) -> Result<()> {
    let part = part_path(dest);
    let mut have = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
    if have > size {
        tokio::fs::remove_file(&part).await?;
        have = 0;
    }
    if have < size {
        let mut req = client.get(url);
        if have > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={have}-"));
        }
        let resp = req.send().await?.error_for_status().with_context(|| format!("descargando {url}"))?;
        // Si el servidor ignora el Range, empieza de cero.
        let append = have > 0 && resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        if !append {
            have = 0;
        }
        let mut out = tokio::fs::OpenOptions::new().create(true).write(true).append(append).truncate(!append).open(&part).await?;
        on_bytes(have);
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::Relaxed) {
                out.flush().await?;
                bail!("descarga cancelada");
            }
            let chunk = chunk?;
            out.write_all(&chunk).await?;
            have += chunk.len() as u64;
            on_bytes(have);
        }
        out.flush().await?;
    }
    if have != size {
        bail!("{}: se esperaban {size} bytes y llegaron {have}", dest.display());
    }
    if let Some(expected) = sha256 {
        let p = part.clone();
        let got = tokio::task::spawn_blocking(move || sha256_file(&p)).await??;
        if !got.eq_ignore_ascii_case(expected) {
            tokio::fs::remove_file(&part).await.ok();
            bail!("{}: el SHA-256 no coincide (se borra para volver a bajarlo)", dest.display());
        }
    }
    tokio::fs::rename(&part, dest).await?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Platform;

    fn catalog() -> Catalog {
        Catalog::parse(include_str!("../../../models.toml")).unwrap()
    }

    #[test]
    fn plan_uses_existing_folder_names() {
        let c = catalog();
        let m = Machine { platform: Platform::MacosArm64, ram_gb: 16 };
        let plan = c.plan(APP_SLOTS, &m, Path::new("/m")).unwrap();
        let dirs: Vec<_> = plan.iter().map(|p| p.dir.to_string_lossy().into_owned()).collect();
        assert_eq!(dirs, ["/m/vad/silero-vad-mlx", "/m/stt_streaming/qwen3-asr-1.7b-4bit-mlx", "/m/llm/lfm3-1.7b-q8"]);
    }

    #[test]
    fn status_requires_derived_tokenizer() {
        let c = catalog();
        let m = Machine { platform: Platform::MacosArm64, ram_gb: 16 };
        let tmp = std::env::temp_dir().join(format!("juanola-models-{}", std::process::id()));
        let plan = c.plan(&["stt_streaming"], &m, &tmp).unwrap();
        let p = &plan[0];
        std::fs::create_dir_all(&p.dir).unwrap();
        for f in &p.entry.files {
            std::fs::write(p.dir.join(f), b"x").unwrap();
        }
        assert!(!p.status().installed, "falta tokenizer.json");
        std::fs::write(p.dir.join("tokenizer.json"), b"{}").unwrap();
        assert!(p.status().installed);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    /// Servidor HTTP mínimo que respeta `Range`, para probar la reanudación.
    async fn serve(body: Vec<u8>) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let (mut sock, _) = listener.accept().await.unwrap();
                let body = body.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let n = sock.read(&mut buf).await.unwrap();
                    let req = String::from_utf8_lossy(&buf[..n]).to_lowercase();
                    let from = req
                        .lines()
                        .find_map(|l| l.strip_prefix("range: bytes="))
                        .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok());
                    let (status, slice) = match from {
                        Some(f) => ("206 Partial Content", &body[f..]),
                        None => ("200 OK", &body[..]),
                    };
                    let head = format!("HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", slice.len());
                    sock.write_all(head.as_bytes()).await.unwrap();
                    sock.write_all(slice).await.unwrap();
                });
            }
        });
        format!("http://{addr}/f")
    }

    #[tokio::test]
    async fn download_resumes_and_checks_sha() {
        let body: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let sha = {
            let mut h = Sha256::new();
            h.update(&body);
            h.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>()
        };
        let url = serve(body.clone()).await;
        let dir = std::env::temp_dir().join(format!("juanola-dl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("model.bin");
        // Medio fichero ya bajado: solo se piden los bytes que faltan.
        std::fs::write(part_path(&dest), &body[..80_000]).unwrap();
        let client = reqwest::Client::new();
        let mut first = None;
        download(&client, &url, &dest, body.len() as u64, Some(&sha), &AtomicBool::new(false), |n| {
            first.get_or_insert(n);
        })
        .await
        .unwrap();
        assert_eq!(first, Some(80_000), "continúa desde el .part");
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        assert!(!part_path(&dest).exists());

        // Un SHA que no coincide borra el .part y falla.
        let bad = dir.join("bad.bin");
        let err = download(&client, &url, &bad, body.len() as u64, Some("00"), &AtomicBool::new(false), |_| {}).await.unwrap_err();
        assert!(err.to_string().contains("SHA-256"));
        assert!(!bad.exists() && !part_path(&bad).exists());

        // Cancelar deja el .part para continuar después.
        let cancelled = dir.join("cancel.bin");
        let cancel = AtomicBool::new(true);
        assert!(download(&client, &url, &cancelled, body.len() as u64, None, &cancel, |_| {}).await.is_err());
        assert!(part_path(&cancelled).exists() && !cancelled.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
