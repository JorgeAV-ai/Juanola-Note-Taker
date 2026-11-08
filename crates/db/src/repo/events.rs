//! Copia local de los eventos de calendario que tienen nota: así la nota
//! conserva su título, hora y asistentes aunque el evento cambie o se borre.

use juanola_core_types::CalendarEvent;
use sqlx::Row;

use crate::{Db, DbError};

pub async fn upsert(db: &Db, e: &CalendarEvent) -> Result<(), DbError> {
    let attendees = serde_json::to_string(&e.attendees).unwrap_or_else(|_| "[]".into());
    sqlx::query(
        "INSERT INTO calendar_events (id, provider, calendar_id, title, start_at, end_at, attendees, conference_url, recurring_event_id, description)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET title = excluded.title, start_at = excluded.start_at, end_at = excluded.end_at,
           attendees = excluded.attendees, conference_url = excluded.conference_url,
           recurring_event_id = excluded.recurring_event_id, description = excluded.description",
    )
    .bind(&e.id)
    .bind(&e.provider)
    .bind(&e.calendar_id)
    .bind(&e.title)
    .bind(e.start.to_rfc3339())
    .bind(e.end.to_rfc3339())
    .bind(attendees)
    .bind(&e.conference_url)
    .bind(&e.recurring_event_id)
    .bind(&e.description)
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn get(db: &Db, id: &str) -> Result<Option<CalendarEvent>, DbError> {
    let row = sqlx::query("SELECT * FROM calendar_events WHERE id = ?").bind(id).fetch_optional(db.pool()).await?;
    let Some(r) = row else { return Ok(None) };
    let parse = |s: String| chrono::DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&chrono::Utc)).unwrap_or_default();
    let attendees: String = r.try_get("attendees")?;
    Ok(Some(CalendarEvent {
        id: r.try_get("id")?,
        provider: r.try_get("provider")?,
        calendar_id: r.try_get("calendar_id")?,
        title: r.try_get("title")?,
        start: parse(r.try_get("start_at")?),
        end: parse(r.try_get("end_at")?),
        attendees: serde_json::from_str(&attendees).unwrap_or_default(),
        conference_url: r.try_get("conference_url")?,
        recurring_event_id: r.try_get("recurring_event_id")?,
        description: r.try_get("description")?,
    }))
}
