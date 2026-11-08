//! Lista los eventos de las próximas 48 h del calendario de macOS.
//! Uso: `cargo run -p juanola-calendar --example list_events`. Desde la
//! terminal macOS pregunta el permiso para la propia terminal.

fn main() {
    #[cfg(target_os = "macos")]
    {
        use juanola_calendar::eventkit;
        let access = eventkit::request_access(std::time::Duration::from_secs(60));
        println!("acceso: {access:?}");
        let now = chrono::Utc::now();
        for e in eventkit::events(now - chrono::Duration::hours(2), now + chrono::Duration::hours(48)) {
            println!(
                "{}  {}  asistentes={} url={:?} recurrente={}",
                e.start.with_timezone(&chrono::Local).format("%a %d %H:%M"),
                e.title,
                e.attendees.len(),
                e.conference_url,
                e.recurring_event_id.is_some()
            );
        }
    }
}
