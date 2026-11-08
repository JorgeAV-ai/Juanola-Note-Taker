//! Carpetas del espacio «My notes». Una nota está como mucho en una carpeta
//! (la tabla `folder_notes` permitiría varias; la UI de momento no).

use juanola_core_types::{Id, Note};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{Db, DbError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Folder {
    pub id: Id,
    pub name: String,
    /// Notas no borradas dentro de la carpeta.
    pub note_count: i64,
}

/// Id del espacio «My notes» del workspace (se crea la primera vez).
async fn my_notes_space(db: &Db, workspace_id: Id) -> Result<String, DbError> {
    let row = sqlx::query("SELECT id FROM spaces WHERE workspace_id = ? AND kind = 'my_notes' LIMIT 1")
        .bind(workspace_id.to_string())
        .fetch_optional(db.pool())
        .await?;
    if let Some(r) = row {
        return Ok(r.try_get("id")?);
    }
    let id = Id::new_v4().to_string();
    sqlx::query("INSERT INTO spaces (id, workspace_id, kind, name) VALUES (?, ?, 'my_notes', 'My notes')")
        .bind(&id)
        .bind(workspace_id.to_string())
        .execute(db.pool())
        .await?;
    Ok(id)
}

pub async fn list(db: &Db, workspace_id: Id) -> Result<Vec<Folder>, DbError> {
    let space = my_notes_space(db, workspace_id).await?;
    let rows = sqlx::query(
        "SELECT f.id, f.name, \
           (SELECT COUNT(*) FROM folder_notes fn JOIN notes n ON n.id = fn.note_id \
            WHERE fn.folder_id = f.id AND n.trashed_at IS NULL) AS note_count \
         FROM folders f WHERE f.space_id = ? ORDER BY f.created_at",
    )
    .bind(space)
    .fetch_all(db.pool())
    .await?;
    rows.iter()
        .map(|r| {
            let id: String = r.try_get("id")?;
            Ok(Folder { id: id.parse().unwrap_or_default(), name: r.try_get("name")?, note_count: r.try_get("note_count")? })
        })
        .collect()
}

pub async fn create(db: &Db, workspace_id: Id, name: &str) -> Result<Folder, DbError> {
    let space = my_notes_space(db, workspace_id).await?;
    let id = Id::new_v4();
    sqlx::query("INSERT INTO folders (id, space_id, name) VALUES (?, ?, ?)")
        .bind(id.to_string())
        .bind(space)
        .bind(name)
        .execute(db.pool())
        .await?;
    Ok(Folder { id, name: name.to_owned(), note_count: 0 })
}

pub async fn rename(db: &Db, id: Id, name: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE folders SET name = ? WHERE id = ?").bind(name).bind(id.to_string()).execute(db.pool()).await?;
    Ok(())
}

/// Borra la carpeta; sus notas siguen existiendo, fuera de cualquier carpeta.
pub async fn delete(db: &Db, id: Id) -> Result<(), DbError> {
    sqlx::query("DELETE FROM folder_notes WHERE folder_id = ?").bind(id.to_string()).execute(db.pool()).await?;
    sqlx::query("DELETE FROM folders WHERE id = ?").bind(id.to_string()).execute(db.pool()).await?;
    Ok(())
}

/// Mueve la nota a `folder` (o la saca de su carpeta con `None`).
pub async fn set_note_folder(db: &Db, note_id: Id, folder: Option<Id>) -> Result<(), DbError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("DELETE FROM folder_notes WHERE note_id = ?").bind(note_id.to_string()).execute(&mut *tx).await?;
    if let Some(f) = folder {
        sqlx::query("INSERT INTO folder_notes (folder_id, note_id) VALUES (?, ?)")
            .bind(f.to_string())
            .bind(note_id.to_string())
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn note_folder(db: &Db, note_id: Id) -> Result<Option<Id>, DbError> {
    let row = sqlx::query("SELECT folder_id FROM folder_notes WHERE note_id = ? LIMIT 1")
        .bind(note_id.to_string())
        .fetch_optional(db.pool())
        .await?;
    Ok(row.and_then(|r| r.try_get::<String, _>("folder_id").ok()).and_then(|s| s.parse().ok()))
}

pub async fn notes(db: &Db, folder: Id) -> Result<Vec<Note>, DbError> {
    let ids: Vec<String> = sqlx::query(
        "SELECT n.id FROM notes n JOIN folder_notes fn ON fn.note_id = n.id \
         WHERE fn.folder_id = ? AND n.trashed_at IS NULL ORDER BY n.created_at DESC",
    )
    .bind(folder.to_string())
    .fetch_all(db.pool())
    .await?
    .iter()
    .map(|r| r.try_get("id"))
    .collect::<Result<_, _>>()?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(n) = super::notes::get(db, id.parse().unwrap_or_default()).await? {
            out.push(n);
        }
    }
    Ok(out)
}
