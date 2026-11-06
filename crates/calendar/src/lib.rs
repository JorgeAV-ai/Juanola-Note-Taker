//! Proveedores de calendario (Google, Microsoft Graph, Apple como extra),
//! caché local, "Coming up" y vínculo de recurrentes.

use chrono::{DateTime, Utc};
use granola_core_types::CalendarEvent;

#[async_trait::async_trait]
pub trait CalendarProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn list_calendars(&self) -> anyhow::Result<Vec<(String, String)>>;
    async fn events(
        &self,
        calendar_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> anyhow::Result<Vec<CalendarEvent>>;
}

/// Filtro de Granola: oculta rechazados, OOO, Focus Time, Working Location.
pub fn is_meeting(ev: &CalendarEvent) -> bool {
    let t = ev.title.to_lowercase();
    let noise = ["out of office", "focus time", "working location"];
    !noise.iter().any(|n| t.contains(n))
        && !ev.attendees.iter().any(|a| a.is_self && a.declined)
}

pub mod providers {
    pub mod google {}
    pub mod microsoft {}
}
