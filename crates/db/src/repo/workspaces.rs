use juanola_core_types::Id;

use crate::{Db, DbError};

/// Workspace personal por defecto (id nulo): hasta que haya cuentas, todas las
/// notas son de él.
pub const PERSONAL: Id = Id::nil();

pub async fn ensure_personal(db: &Db) -> Result<Id, DbError> {
    sqlx::query("INSERT OR IGNORE INTO workspaces (id, name, is_personal) VALUES (?, 'Personal', 1)")
        .bind(PERSONAL.to_string())
        .execute(db.pool())
        .await?;
    Ok(PERSONAL)
}
