//! Plantillas del usuario. Las integradas viven en código (`juanola-enhance`)
//! y la app las combina con estas; aquí solo se guardan las del usuario.

use juanola_core_types::{Id, Template, parse_structure};
use sqlx::Row;

use crate::{Db, DbError};

const COLS: &str = "id, name, purpose, style, structure, shared_with_workspace";

fn row_to_template(r: &sqlx::sqlite::SqliteRow) -> Result<Template, DbError> {
    let id: String = r.try_get("id")?;
    let structure: String = r.try_get("structure")?;
    let shared: i64 = r.try_get("shared_with_workspace")?;
    Ok(Template {
        id: id.parse().map_err(|_| DbError::Corrupt("templates.id".into()))?,
        name: r.try_get("name")?,
        purpose: r.try_get("purpose")?,
        style: r.try_get("style")?,
        structure: parse_structure(&structure),
        is_builtin: false,
        shared_with_workspace: shared != 0,
    })
}

/// Plantillas del usuario por nombre.
pub async fn list(db: &Db) -> Result<Vec<Template>, DbError> {
    let rows = sqlx::query(&format!("SELECT {COLS} FROM templates WHERE is_builtin = 0 ORDER BY name COLLATE NOCASE"))
        .fetch_all(db.pool())
        .await?;
    rows.iter().map(row_to_template).collect()
}

pub async fn get(db: &Db, id: Id) -> Result<Option<Template>, DbError> {
    let row = sqlx::query(&format!("SELECT {COLS} FROM templates WHERE id = ? AND is_builtin = 0"))
        .bind(id.to_string())
        .fetch_optional(db.pool())
        .await?;
    row.as_ref().map(row_to_template).transpose()
}

/// Crea o sustituye una plantilla del usuario. Rechaza las que no pasan
/// [`Template::validate`] y las marcadas como integradas.
pub async fn upsert(db: &Db, t: &Template) -> Result<(), DbError> {
    if t.is_builtin {
        return Err(DbError::Invalid("las plantillas integradas no se guardan".into()));
    }
    t.validate().map_err(|e| DbError::Invalid(e.to_string()))?;
    let structure = serde_json::to_string(&t.structure).map_err(|e| DbError::Invalid(e.to_string()))?;
    sqlx::query(
        "INSERT INTO templates (id, name, purpose, style, structure, is_builtin, shared_with_workspace) VALUES (?, ?, ?, ?, ?, 0, ?)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, purpose = excluded.purpose, style = excluded.style,
           structure = excluded.structure, shared_with_workspace = excluded.shared_with_workspace",
    )
    .bind(t.id.to_string())
    .bind(&t.name)
    .bind(&t.purpose)
    .bind(&t.style)
    .bind(structure)
    .bind(t.shared_with_workspace as i64)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Borra la plantilla y la quita de las notas que la usaban (vuelven a Auto).
pub async fn delete(db: &Db, id: Id) -> Result<(), DbError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("UPDATE notes SET template_id = NULL WHERE template_id = ?").bind(id.to_string()).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM templates WHERE id = ? AND is_builtin = 0").bind(id.to_string()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
