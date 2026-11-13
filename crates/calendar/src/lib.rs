//! Proveedores de calendario y la lógica sobre los eventos:
//! qué cuenta como reunión, el enlace de la videollamada, qué evento está en
//! curso cuando se detecta una llamada y cuándo avisar antes de empezar.
//!
//! Proveedores: EventKit en macOS ([`eventkit`], lee todas las cuentas que el
//! usuario tenga en Calendario: iCloud, Google, Exchange…). Google y Microsoft
//! Graph directos quedan pendientes (necesitan un cliente OAuth).

use chrono::{DateTime, Duration, Utc};
use juanola_core_types::CalendarEvent;

#[cfg(target_os = "macos")]
pub mod eventkit;

#[async_trait::async_trait]
pub trait CalendarProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn list_calendars(&self) -> anyhow::Result<Vec<(String, String)>>;
    async fn events(&self, calendar_id: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> anyhow::Result<Vec<CalendarEvent>>;
}

/// Permiso de acceso al calendario del sistema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Granted,
    Denied,
    NotDetermined,
    /// Solo escritura (macOS 14+): no deja leer eventos.
    WriteOnly,
    Unsupported,
}

/// Filtro de reuniones: oculta rechazados, OOO, Focus Time, Working Location.
pub fn is_meeting(ev: &CalendarEvent) -> bool {
    let t = ev.title.to_lowercase();
    let noise = ["out of office", "focus time", "working location", "fuera de la oficina", "tiempo de concentración"];
    !noise.iter().any(|n| t.contains(n)) && !ev.attendees.iter().any(|a| a.is_self && a.declined)
}

/// Dominios de videollamada que se reconocen en la URL, el lugar o las notas.
/// Servicios de videollamada: host (o dominio padre, p. ej. `acme.zoom.us`) y
/// prefijo de la ruta.
const CONFERENCE_HOSTS: &[(&str, &str)] = &[
    ("meet.google.com", "/"),
    ("zoom.us", "/j/"),
    ("zoom.us", "/my/"),
    ("zoom.us", "/w/"),
    ("teams.microsoft.com", "/l/meetup-join"),
    ("teams.microsoft.com", "/meet/"),
    ("teams.live.com", "/meet/"),
    ("webex.com", "/"),
    ("whereby.com", "/"),
    ("meet.jit.si", "/"),
    ("around.co", "/"),
    ("facetime.apple.com", "/"),
    ("app.slack.com", "/huddle"),
];

/// `true` si `url` es `https://` a uno de [`CONFERENCE_HOSTS`]. Se compara el
/// host entero (o un sufijo con punto), no una subcadena: un enlace
/// `https://otro.example/?r=meet.google.com/x` no cuenta.
pub fn is_conference_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let split = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(split);
    if authority.contains(['@', '\\']) {
        return false;
    }
    let host = authority.split(':').next().unwrap_or("").to_ascii_lowercase();
    CONFERENCE_HOSTS.iter().any(|(h, prefix)| {
        let host_ok = host == *h || host.strip_suffix(h).is_some_and(|sub| sub.ends_with('.') && sub.len() > 1);
        host_ok && path.starts_with(prefix)
    })
}

/// Primer enlace de videollamada en los textos del evento (URL, lugar, notas).
pub fn find_conference_url<'a>(texts: impl IntoIterator<Item = &'a str>) -> Option<String> {
    for text in texts {
        for (i, _) in text.match_indices("https://") {
            let rest = &text[i..];
            let end = rest.find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | ')' | ']' | '|')).unwrap_or(rest.len());
            let url = rest[..end].trim_end_matches(['.', ',', ';']);
            if is_conference_url(url) {
                return Some(url.to_string());
            }
        }
    }
    None
}

/// Evento al que pertenece una llamada detectada en `now`: uno que empieza en
/// los próximos 5 min o que empezó hace menos de 15 (o sigue en curso). Si hay
/// varios, el que empieza más cerca de `now`.
pub fn event_for_call(events: &[CalendarEvent], now: DateTime<Utc>) -> Option<&CalendarEvent> {
    events
        .iter()
        .filter(|e| is_meeting(e))
        .filter(|e| {
            let from = e.start - Duration::minutes(5);
            let to = e.end.max(e.start + Duration::minutes(15));
            from <= now && now <= to
        })
        .min_by_key(|e| (e.start - now).num_seconds().abs())
}

/// Eventos por los que avisar en `now`: 1 minuto antes de
/// empezar y solo si tienen 2 o más asistentes (el usuario incluido).
pub fn due_reminders(events: &[CalendarEvent], now: DateTime<Utc>) -> Vec<&CalendarEvent> {
    events
        .iter()
        .filter(|e| is_meeting(e) && e.attendees.len() >= 2)
        .filter(|e| {
            let until = e.start - now;
            until <= Duration::minutes(1) && until > -Duration::minutes(1)
        })
        .collect()
}

/// Eventos para «Coming up»: los que no han terminado, en orden, sin ruido.
pub fn coming_up(mut events: Vec<CalendarEvent>, now: DateTime<Utc>) -> Vec<CalendarEvent> {
    events.retain(|e| e.end > now && is_meeting(e));
    events.sort_by_key(|e| e.start);
    events
}

pub mod providers {
    pub mod google {}
    pub mod microsoft {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use juanola_core_types::Attendee;

    fn at(min: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-09T10:00:00Z").unwrap().with_timezone(&Utc) + Duration::minutes(min)
    }

    fn ev(id: &str, start: i64, len: i64, attendees: usize) -> CalendarEvent {
        CalendarEvent {
            id: id.into(),
            provider: "test".into(),
            calendar_id: "c".into(),
            title: id.into(),
            start: at(start),
            end: at(start + len),
            attendees: (0..attendees)
                .map(|i| Attendee { email: format!("p{i}@x.com"), name: None, declined: false, is_self: i == 0 })
                .collect(),
            conference_url: None,
            recurring_event_id: None,
            description: None,
        }
    }

    #[test]
    fn finds_conference_links() {
        let notes = "Únete con Google Meet: https://meet.google.com/abc-defg-hij\nO llama al +34…";
        assert_eq!(find_conference_url([notes]).as_deref(), Some("https://meet.google.com/abc-defg-hij"));
        let teams = "<https://teams.microsoft.com/l/meetup-join/19%3ameeting_X%40thread.v2/0?context=%7b%7d>";
        assert!(find_conference_url(["", teams]).unwrap().starts_with("https://teams.microsoft.com/l/meetup-join/"));
        assert_eq!(
            find_conference_url(["https://acme.zoom.us/j/123456789?pwd=x."]).as_deref(),
            Some("https://acme.zoom.us/j/123456789?pwd=x")
        );
        assert_eq!(find_conference_url(["https://example.com/agenda", "Sala 3"]), None);
        // El host se compara entero: ni subcadenas, ni userinfo, ni http.
        assert_eq!(find_conference_url(["https://evil.example/login?r=meet.google.com/abc"]), None);
        assert_eq!(find_conference_url(["https://meet.google.com.evil.example/abc"]), None);
        assert_eq!(find_conference_url(["https://meet.google.com@evil.example/abc"]), None);
        assert_eq!(find_conference_url(["https://notzoom.us/j/1"]), None);
        assert_eq!(find_conference_url(["http://meet.google.com/abc"]), None);
        assert_eq!(
            find_conference_url(["mal: https://x.example/?u=zoom.us/j/1 bien: https://Meet.Google.com/abc"]).as_deref(),
            Some("https://Meet.Google.com/abc")
        );
    }

    #[test]
    fn declined_and_noise_are_hidden() {
        let mut e = ev("Focus time", 0, 60, 1);
        assert!(!is_meeting(&e));
        e.title = "Sync".into();
        assert!(is_meeting(&e));
        e.attendees[0].declined = true;
        assert!(!is_meeting(&e));
    }

    #[test]
    fn call_matches_event_window() {
        let events = vec![ev("a", 0, 30, 2), ev("b", 60, 30, 2)];
        assert_eq!(event_for_call(&events, at(-4)).map(|e| e.id.as_str()), Some("a"));
        assert_eq!(event_for_call(&events, at(29)).map(|e| e.id.as_str()), Some("a"));
        assert!(event_for_call(&events, at(40)).is_none());
        assert_eq!(event_for_call(&events, at(57)).map(|e| e.id.as_str()), Some("b"));
        // Evento corto: se acepta hasta 15 min después de empezar.
        let short = vec![ev("c", 0, 5, 2)];
        assert_eq!(event_for_call(&short, at(12)).map(|e| e.id.as_str()), Some("c"));
    }

    #[test]
    fn reminders_one_minute_before_with_two_attendees() {
        let events = vec![ev("solo", 1, 30, 1), ev("team", 1, 30, 3), ev("later", 10, 30, 3)];
        let ids: Vec<_> = due_reminders(&events, at(0)).iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["team"]);
        assert!(due_reminders(&events, at(-2)).is_empty());
    }

    #[test]
    fn coming_up_sorts_and_drops_finished() {
        let events = vec![ev("b", 60, 30, 2), ev("past", -90, 30, 2), ev("a", -10, 30, 2)];
        let ids: Vec<_> = coming_up(events, at(0)).into_iter().map(|e| e.id).collect();
        assert_eq!(ids, ["a", "b"]);
    }
}
