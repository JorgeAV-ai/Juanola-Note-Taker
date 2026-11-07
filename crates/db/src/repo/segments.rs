use juanola_core_types::{Channel, Id, Segment};
use sqlx::Row;

use crate::{Db, DbError};

fn channel_str(c: Channel) -> &'static str {
    match c {
        Channel::Mic => "mic",
        Channel::System => "system",
    }
}

/// Guarda un segmento final de la transcripción.
pub async fn insert(db: &Db, s: &Segment) -> Result<(), DbError> {
    sqlx::query(
        "INSERT OR REPLACE INTO segments (id, note_id, channel, speaker_id, start_s, end_s, text, confidence, is_final) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(s.id.to_string())
    .bind(s.note_id.to_string())
    .bind(channel_str(s.channel))
    .bind(s.speaker_id.map(|i| i.to_string()))
    .bind(s.start)
    .bind(s.end)
    .bind(&s.text)
    .bind(s.confidence)
    .bind(s.is_final)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Segmentos de una nota en orden temporal.
pub async fn list(db: &Db, note_id: Id) -> Result<Vec<Segment>, DbError> {
    let rows = sqlx::query(
        "SELECT id, note_id, channel, start_s, end_s, text, confidence, is_final FROM segments WHERE note_id = ? ORDER BY start_s",
    )
    .bind(note_id.to_string())
    .fetch_all(db.pool())
    .await?;
    rows.iter()
        .map(|r| {
            let id: String = r.try_get("id")?;
            let channel: String = r.try_get("channel")?;
            Ok(Segment {
                id: id.parse().unwrap_or_default(),
                note_id,
                channel: if channel == "mic" { Channel::Mic } else { Channel::System },
                speaker_id: None,
                start: r.try_get("start_s")?,
                end: r.try_get("end_s")?,
                text: r.try_get("text")?,
                confidence: r.try_get("confidence")?,
                is_final: r.try_get("is_final")?,
            })
        })
        .collect()
}

/// Hora de fin del último segmento (para continuar una grabación tras Resume).
pub async fn last_end(db: &Db, note_id: Id) -> Result<f64, DbError> {
    let v: Option<f64> = sqlx::query_scalar("SELECT MAX(end_s) FROM segments WHERE note_id = ?")
        .bind(note_id.to_string())
        .fetch_one(db.pool())
        .await?;
    Ok(v.unwrap_or(0.0))
}
