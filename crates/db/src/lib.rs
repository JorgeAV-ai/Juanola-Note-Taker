//! Persistencia local en SQLite (sqlx) con migraciones embebidas.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Abre (o crea) la base de datos y aplica las migraciones de `./migrations`.
    pub async fn open(path: &Path) -> Result<Self, DbError> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new().max_connections(8).connect_with(opts).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

pub mod repo;

#[cfg(test)]
mod tests {
    use super::*;
    use juanola_core_types::{Channel, NoteStatus, Segment};

    #[tokio::test]
    async fn notes_and_segments_roundtrip() {
        let dir = std::env::temp_dir().join(format!("juanola-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir.join("t.sqlite")).await.unwrap();
        let ws = repo::workspaces::ensure_personal(&db).await.unwrap();
        repo::workspaces::ensure_personal(&db).await.unwrap(); // idempotente
        let n = repo::notes::create(&db, ws, "Reunión").await.unwrap();
        repo::notes::set_title(&db, n.id, "Reunión semanal").await.unwrap();
        repo::notes::set_status(&db, n.id, NoteStatus::Ready).await.unwrap();
        repo::notes::set_enhanced_doc(&db, n.id, Some(&serde_json::json!({"sections": []}))).await.unwrap();
        let seg = Segment {
            id: uuid::Uuid::new_v4(),
            note_id: n.id,
            channel: Channel::System,
            speaker_id: None,
            start: 1.5,
            end: 3.0,
            text: "hola".into(),
            confidence: None,
            is_final: true,
        };
        repo::segments::insert(&db, &seg).await.unwrap();
        let got = repo::notes::get(&db, n.id).await.unwrap().unwrap();
        assert_eq!(got.title, "Reunión semanal");
        assert_eq!(got.status, NoteStatus::Ready);
        assert!(got.enhanced_doc.is_some());
        assert_eq!(repo::notes::list_recent(&db, 10).await.unwrap().len(), 1);
        let segs = repo::segments::list(&db, n.id).await.unwrap();
        assert_eq!((segs.len(), segs[0].channel, segs[0].text.as_str()), (1, Channel::System, "hola"));
        assert_eq!(repo::segments::last_end(&db, n.id).await.unwrap(), 3.0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
