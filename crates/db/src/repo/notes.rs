use chrono::{DateTime, Utc};
use juanola_core_types::{Id, Note, NoteStatus};
use sqlx::Row;

use crate::{Db, DbError};

fn status_str(s: NoteStatus) -> &'static str {
    match s {
        NoteStatus::Draft => "draft",
        NoteStatus::Transcribing => "transcribing",
        NoteStatus::Enhancing => "enhancing",
        NoteStatus::Ready => "ready",
        NoteStatus::Failed => "failed",
    }
}

fn status_from(s: &str) -> NoteStatus {
    match s {
        "transcribing" => NoteStatus::Transcribing,
        "enhancing" => NoteStatus::Enhancing,
        "ready" => NoteStatus::Ready,
        "failed" => NoteStatus::Failed,
        _ => NoteStatus::Draft,
    }
}

fn parse_time(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now())
}

fn row_to_note(r: &sqlx::sqlite::SqliteRow) -> Result<Note, DbError> {
    let id: String = r.try_get("id")?;
    let ws: String = r.try_get("workspace_id")?;
    let raw: String = r.try_get("raw_doc")?;
    let enhanced: Option<String> = r.try_get("enhanced_doc")?;
    let status: String = r.try_get("status")?;
    let created: String = r.try_get("created_at")?;
    let updated: String = r.try_get("updated_at")?;
    let trashed: Option<String> = r.try_get("trashed_at")?;
    let template: Option<String> = r.try_get("template_id")?;
    Ok(Note {
        id: id.parse().unwrap_or_default(),
        workspace_id: ws.parse().unwrap_or_default(),
        title: r.try_get("title")?,
        raw_doc: serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({"type": "doc", "content": []})),
        enhanced_doc: enhanced.and_then(|e| serde_json::from_str(&e).ok()),
        template_id: template.and_then(|t| t.parse().ok()),
        status: status_from(&status),
        calendar_event_id: r.try_get("calendar_event_id")?,
        created_at: parse_time(&created),
        updated_at: parse_time(&updated),
        trashed_at: trashed.as_deref().map(parse_time),
    })
}

const COLS: &str =
    "id, workspace_id, title, raw_doc, enhanced_doc, template_id, status, calendar_event_id, created_at, updated_at, trashed_at";

pub async fn create(db: &Db, workspace_id: Id, title: &str) -> Result<Note, DbError> {
    let id = Id::new_v4();
    sqlx::query("INSERT INTO notes (id, workspace_id, title) VALUES (?, ?, ?)")
        .bind(id.to_string())
        .bind(workspace_id.to_string())
        .bind(title)
        .execute(db.pool())
        .await?;
    get(db, id).await?.ok_or(DbError::Sqlx(sqlx::Error::RowNotFound))
}

pub async fn get(db: &Db, id: Id) -> Result<Option<Note>, DbError> {
    let row = sqlx::query(&format!("SELECT {COLS} FROM notes WHERE id = ?")).bind(id.to_string()).fetch_optional(db.pool()).await?;
    row.as_ref().map(row_to_note).transpose()
}

/// Notas no borradas, de la más reciente a la más antigua.
pub async fn list_recent(db: &Db, limit: i64) -> Result<Vec<Note>, DbError> {
    let rows = sqlx::query(&format!("SELECT {COLS} FROM notes WHERE trashed_at IS NULL ORDER BY created_at DESC LIMIT ?"))
        .bind(limit)
        .fetch_all(db.pool())
        .await?;
    rows.iter().map(row_to_note).collect()
}

async fn touch(db: &Db, sql: &str, id: Id, value: Option<String>) -> Result<(), DbError> {
    let q = sqlx::query(sql);
    let q = match value {
        Some(v) => q.bind(v),
        None => q.bind(None::<String>),
    };
    q.bind(id.to_string()).execute(db.pool()).await?;
    Ok(())
}

pub async fn set_title(db: &Db, id: Id, title: &str) -> Result<(), DbError> {
    touch(db, "UPDATE notes SET title = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?", id, Some(title.to_owned()))
        .await
}

pub async fn set_raw_doc(db: &Db, id: Id, doc: &serde_json::Value) -> Result<(), DbError> {
    touch(db, "UPDATE notes SET raw_doc = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?", id, Some(doc.to_string()))
        .await
}

pub async fn set_enhanced_doc(db: &Db, id: Id, doc: Option<&serde_json::Value>) -> Result<(), DbError> {
    touch(
        db,
        "UPDATE notes SET enhanced_doc = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        id,
        doc.map(|d| d.to_string()),
    )
    .await
}

pub async fn set_status(db: &Db, id: Id, status: NoteStatus) -> Result<(), DbError> {
    touch(db, "UPDATE notes SET status = ? WHERE id = ?", id, Some(status_str(status).to_owned())).await
}

/// Manda la nota a la papelera; `restore` la recupera.
pub async fn trash(db: &Db, id: Id) -> Result<(), DbError> {
    sqlx::query("UPDATE notes SET trashed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?")
        .bind(id.to_string())
        .execute(db.pool())
        .await?;
    Ok(())
}

pub async fn restore(db: &Db, id: Id) -> Result<(), DbError> {
    sqlx::query("UPDATE notes SET trashed_at = NULL WHERE id = ?").bind(id.to_string()).execute(db.pool()).await?;
    Ok(())
}

/// Notas en la papelera, la última borrada primero.
pub async fn list_trashed(db: &Db) -> Result<Vec<Note>, DbError> {
    let rows = sqlx::query(&format!("SELECT {COLS} FROM notes WHERE trashed_at IS NOT NULL ORDER BY trashed_at DESC"))
        .fetch_all(db.pool())
        .await?;
    rows.iter().map(row_to_note).collect()
}

/// Borra la nota para siempre (con sus segmentos, por `ON DELETE CASCADE`).
pub async fn delete_forever(db: &Db, id: Id) -> Result<(), DbError> {
    sqlx::query("DELETE FROM notes WHERE id = ? AND trashed_at IS NOT NULL").bind(id.to_string()).execute(db.pool()).await?;
    Ok(())
}

/// Nota (no borrada) de un evento de calendario, si ya existe.
pub async fn find_by_event(db: &Db, event_id: &str) -> Result<Option<Note>, DbError> {
    let row = sqlx::query(&format!(
        "SELECT {COLS} FROM notes WHERE calendar_event_id = ? AND trashed_at IS NULL ORDER BY created_at DESC LIMIT 1"
    ))
    .bind(event_id)
    .fetch_optional(db.pool())
    .await?;
    row.as_ref().map(row_to_note).transpose()
}

pub async fn set_calendar_event(db: &Db, id: Id, event_id: Option<&str>) -> Result<(), DbError> {
    sqlx::query("UPDATE notes SET calendar_event_id = ? WHERE id = ?").bind(event_id).bind(id.to_string()).execute(db.pool()).await?;
    Ok(())
}
