//! Calendario de macOS vía EventKit. Lee los eventos de todas las cuentas que
//! el usuario tenga en la app Calendario. Necesita `NSCalendarsFullAccessUsageDescription`
//! (macOS 14+) o `NSCalendarsUsageDescription` (13) en el Info.plist de la app.
//!
//! Cada llamada crea su propio `EKEventStore` (no es `Send`); cuesta unos
//! milisegundos y evita compartir un objeto de Objective-C entre hilos.

use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use chrono::{DateTime, TimeZone, Utc};
use juanola_core_types::{Attendee, CalendarEvent};
use objc2::runtime::Bool;
use objc2::sel;
use objc2_event_kit::{EKAuthorizationStatus, EKEntityType, EKEvent, EKEventStore, EKParticipant, EKParticipantStatus};
use objc2_foundation::{NSDate, NSError, NSObjectProtocol};

use crate::{Access, find_conference_url};

pub const PROVIDER: &str = "apple";

pub fn access() -> Access {
    // SAFETY: método de clase sin efectos.
    let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
    match status {
        EKAuthorizationStatus::FullAccess => Access::Granted,
        EKAuthorizationStatus::NotDetermined => Access::NotDetermined,
        EKAuthorizationStatus::WriteOnly => Access::WriteOnly,
        _ => Access::Denied,
    }
}

/// Pide acceso de lectura y espera la respuesta del usuario (hasta `timeout`).
/// Devuelve el estado final. Si ya estaba decidido, macOS responde al momento.
pub fn request_access(timeout: Duration) -> Access {
    let (tx, rx) = mpsc::channel::<bool>();
    // SAFETY: `EKEventStore` se crea y usa en este hilo; el bloque solo envía por un canal.
    unsafe {
        let store = EKEventStore::new();
        let block = RcBlock::new(move |granted: Bool, _err: *mut NSError| {
            let _ = tx.send(granted.as_bool());
        });
        if store.respondsToSelector(sel!(requestFullAccessToEventsWithCompletion:)) {
            store.requestFullAccessToEventsWithCompletion(RcBlock::as_ptr(&block));
        } else {
            #[allow(deprecated)]
            store.requestAccessToEntityType_completion(EKEntityType::Event, RcBlock::as_ptr(&block));
        }
    }
    match rx.recv_timeout(timeout) {
        Ok(_) => access(),
        Err(_) => access(),
    }
}

fn to_utc(date: &NSDate) -> DateTime<Utc> {
    let secs = date.timeIntervalSince1970();
    Utc.timestamp_opt(secs.floor() as i64, ((secs.fract()) * 1e9) as u32).single().unwrap_or_default()
}

fn attendee(p: &EKParticipant) -> Attendee {
    // SAFETY: getters de solo lectura sobre un objeto vivo.
    unsafe {
        let url = p.URL().absoluteString().map(|s| s.to_string()).unwrap_or_default();
        let email = url.strip_prefix("mailto:").unwrap_or(&url).to_string();
        Attendee {
            email,
            name: p.name().map(|n| n.to_string()),
            declined: p.participantStatus() == EKParticipantStatus::Declined,
            is_self: p.isCurrentUser(),
        }
    }
}

fn convert(e: &EKEvent) -> Option<CalendarEvent> {
    // SAFETY: getters de solo lectura sobre un objeto vivo.
    unsafe {
        if e.isAllDay() {
            return None;
        }
        let start = to_utc(&e.startDate());
        let end = to_utc(&e.endDate());
        let item_id = e.calendarItemIdentifier().to_string();
        let calendar_id = e.calendar().map(|c| c.calendarIdentifier().to_string()).unwrap_or_default();
        let attendees: Vec<Attendee> = e.attendees().map(|a| a.iter().map(|p| attendee(&p)).collect()).unwrap_or_default();
        let url = e.URL().and_then(|u| u.absoluteString()).map(|s| s.to_string());
        let location = e.location().map(|s| s.to_string());
        let notes = e.notes().map(|s| s.to_string());
        let conference_url = find_conference_url([url.as_deref(), location.as_deref(), notes.as_deref()].into_iter().flatten());
        // El identificador externo (el UID del servidor) sobrevive a las
        // resincronizaciones; `calendarItemIdentifier` puede cambiar.
        let uid = e.calendarItemExternalIdentifier().map(|s| s.to_string()).filter(|s| !s.is_empty()).unwrap_or(item_id);
        let recurring = e.hasRecurrenceRules();
        let recurring_event_id = recurring.then(|| uid.clone());
        // Las repeticiones comparten UID: la fecha original de cada una (no
        // cambia aunque se mueva esa repetición) las distingue. Un evento
        // suelto conserva su id aunque se cambie de hora, y con él su nota.
        let id = if recurring {
            let occurrence = e.occurrenceDate().map(|d| to_utc(&d)).unwrap_or(start);
            format!("{PROVIDER}:{uid}:{}", occurrence.timestamp())
        } else {
            format!("{PROVIDER}:{uid}")
        };
        Some(CalendarEvent {
            id,
            provider: PROVIDER.into(),
            calendar_id,
            title: e.title().to_string(),
            start,
            end,
            attendees,
            conference_url,
            recurring_event_id,
            description: notes,
        })
    }
}

/// Eventos (sin los de todo el día) que se solapan con `[from, to)`.
/// Vacío si no hay permiso.
pub fn events(from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<CalendarEvent> {
    if access() != Access::Granted {
        return Vec::new();
    }
    // SAFETY: todo ocurre en este hilo con objetos creados aquí.
    unsafe {
        let store = EKEventStore::new();
        let start = NSDate::dateWithTimeIntervalSince1970(from.timestamp() as f64);
        let end = NSDate::dateWithTimeIntervalSince1970(to.timestamp() as f64);
        let predicate = store.predicateForEventsWithStartDate_endDate_calendars(&start, &end, None);
        let found = store.eventsMatchingPredicate(&predicate);
        // Un evento corrupto de un calendario sincronizado (título o fecha en
        // nil) paniquea en los getters de objc2: saltarlo en vez de tumbar el
        // hilo de detección, que no tiene supervisión.
        let mut out: Vec<CalendarEvent> =
            found.iter().filter_map(|e| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| convert(&e))).ok().flatten()).collect();
        out.sort_by_key(|e| e.start);
        // Una invitación puede estar en dos calendarios (dos cuentas) con el mismo UID.
        let mut seen = std::collections::HashSet::new();
        out.retain(|e| seen.insert(e.id.clone()));
        out
    }
}

/// Calendarios de eventos visibles: (identificador, título, cuenta).
pub fn calendars() -> Vec<(String, String, String)> {
    if access() != Access::Granted {
        return Vec::new();
    }
    // SAFETY: objetos creados y usados en este hilo.
    unsafe {
        let store = EKEventStore::new();
        store
            .calendarsForEntityType(EKEntityType::Event)
            .iter()
            .filter_map(|c| {
                // Mismo motivo que en `events`: un calendario con campos nil
                // no debe tumbar el diagnóstico.
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let source = c.source().map(|s| s.title().to_string()).unwrap_or_default();
                    (c.calendarIdentifier().to_string(), c.title().to_string(), source)
                }))
                .ok()
            })
            .collect()
    }
}
