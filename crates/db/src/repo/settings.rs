//! Ajustes clave-valor de la app (valores JSON).

use serde::{de::DeserializeOwned, Serialize};

use crate::{Db, DbError};

pub async fn get<T: DeserializeOwned>(db: &Db, key: &str) -> Result<Option<T>, DbError> {
    let value: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?").bind(key).fetch_optional(db.pool()).await?;
    Ok(value.and_then(|v| serde_json::from_str(&v).ok()))
}

pub async fn set<T: Serialize>(db: &Db, key: &str, value: &T) -> Result<(), DbError> {
    let json = serde_json::to_string(value).map_err(|e| DbError::Sqlx(sqlx::Error::Encode(e.into())))?;
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(key)
        .bind(json)
        .execute(db.pool())
        .await?;
    Ok(())
}
