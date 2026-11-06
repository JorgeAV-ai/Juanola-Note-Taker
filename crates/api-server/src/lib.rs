//! API REST local equivalente a la de Granola: notes, search, transcript,
//! folders, spaces, webhooks.

use axum::{routing::get, Router};

pub fn router() -> Router {
    Router::new()
        .route("/v1/health", get(|| async { "ok" }))
        .route("/v1/notes", get(|| async { "[]" }))
        .route("/v1/folders", get(|| async { "[]" }))
        .route("/v1/spaces", get(|| async { "[]" }))
}
