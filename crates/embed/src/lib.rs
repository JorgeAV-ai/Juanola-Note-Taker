//! Embeddings de texto para RAG en Chat y búsqueda semántica.

#[async_trait::async_trait]
pub trait Embedder: Send + Sync {
    fn dimension(&self) -> usize;
    async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>>;
}

/// Trocea texto en chunks de ~`max_chars` respetando saltos de párrafo.
pub fn chunk(text: &str, max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for para in text.split("\n\n") {
        if !cur.is_empty() && cur.len() + para.len() > max_chars {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push_str("\n\n");
        }
        cur.push_str(para);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
