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

pub mod repo {
    //! Repositorios por agregado. Se rellenan en la fase 1.
    pub mod notes {}
    pub mod segments {}
    pub mod folders {}
    pub mod people {}
    pub mod templates {}
    pub mod recipes {}
    pub mod chats {}
    pub mod settings {}
}
