//! Búsqueda: texto completo (tantivy) + vectorial (sqlite-vec/usearch). Fase 3.

use granola_core_types::Id;

#[derive(Debug, Clone)]
pub struct Hit {
    pub note_id: Id,
    pub score: f32,
    pub snippet: String,
}

#[async_trait::async_trait]
pub trait SearchIndex: Send + Sync {
    async fn index_note(&self, note_id: Id, title: &str, body: &str) -> anyhow::Result<()>;
    async fn query(&self, text: &str, limit: usize) -> anyhow::Result<Vec<Hit>>;
}
