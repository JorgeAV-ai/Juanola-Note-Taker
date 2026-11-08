//! Detección de reuniones y avisos, como Granola:
//!
//! - **Ad hoc**: otra app abre el micrófono (Zoom, Teams, Meet en el
//!   navegador…) → «Meeting detected» con «Take Notes». Si hay un evento del
//!   calendario en curso, la nota se asocia a él.
//! - **Calendario**: 1 minuto antes de un evento con 2+ asistentes.
//! - **Fin de la llamada**: si la grabación empezó desde un aviso y esa app
//!   suelta el micrófono, se avisa al frontend para que pare.
//!
//! Los avisos son una ventana propia (sin marco, siempre encima, arriba a la
//! derecha), no notificaciones del sistema. Un solo hilo hace todo el sondeo.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use juanola_calendar::Access;
use juanola_core_types::CalendarEvent;
use juanola_detect::{notification_title, DetectEvent, DetectedApp, MicWatcher, WatcherConfig};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::state::AppState;

pub const WINDOW: &str = "notification";
const WIDTH: f64 = 384.0;
const HEIGHT: f64 = 124.0;

/// Claves en la tabla `settings`.
pub const KEY_MEETINGS: &str = "detect.meetings";
pub const KEY_REMINDERS: &str = "detect.calendar_reminders";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DetectionSettings {
    /// Avisar cuando otra app abre el micrófono.
    pub meetings: bool,
    /// Avisar 1 minuto antes de los eventos del calendario.
    pub calendar_reminders: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// Llamada detectada por el micrófono (línea discontinua).
    Adhoc,
    /// Evento del calendario a punto de empezar (barra sólida).
    Calendar,
}

/// Lo que muestra la ventana de aviso.
#[derive(Debug, Clone, Serialize)]
pub struct Notification {
    pub kind: NotificationKind,
    /// «Meeting detected», «Huddle detected», «Call detected» o el título del evento.
    pub title: String,
    /// App detectada o «Starts in 1 min».
    pub body: String,
    pub app: Option<DetectedApp>,
    pub event: Option<CalendarEvent>,
}

/// Lo que recibe la ventana principal al pulsar «Take Notes».
#[derive(Debug, Clone, Serialize)]
pub struct TakeNotes {
    pub note_id: juanola_core_types::Id,
}

pub struct Detection {
    meetings: AtomicBool,
    reminders: AtomicBool,
    /// Aviso visible ahora mismo.
    current: Mutex<Option<Notification>>,
    /// App cuya llamada se está grabando (si la grabación salió de un aviso).
    origin_app: Mutex<Option<String>>,
    /// Eventos del calendario de las próximas horas.
    events: Mutex<Vec<CalendarEvent>>,
    /// Eventos por los que ya se avisó (no se repite).
    reminded: Mutex<HashSet<String>>,
}

impl Detection {
    pub fn new(settings: DetectionSettings) -> Self {
        Self {
            meetings: AtomicBool::new(settings.meetings),
            reminders: AtomicBool::new(settings.calendar_reminders),
            current: Mutex::new(None),
            origin_app: Mutex::new(None),
            events: Mutex::new(Vec::new()),
            reminded: Mutex::new(HashSet::new()),
        }
    }

    pub fn settings(&self) -> DetectionSettings {
        DetectionSettings {
            meetings: self.meetings.load(Ordering::Relaxed),
            calendar_reminders: self.reminders.load(Ordering::Relaxed),
        }
    }

    pub fn set_settings(&self, s: DetectionSettings) {
        self.meetings.store(s.meetings, Ordering::Relaxed);
        self.reminders.store(s.calendar_reminders, Ordering::Relaxed);
    }

    pub fn current(&self) -> Option<Notification> {
        self.current.lock().ok()?.clone()
    }

    pub fn take_current(&self) -> Option<Notification> {
        self.current.lock().ok()?.take()
    }

    pub fn set_origin_app(&self, bundle_id: Option<String>) {
        if let Ok(mut o) = self.origin_app.lock() {
            *o = bundle_id;
        }
    }

    pub fn cached_events(&self) -> Vec<CalendarEvent> {
        self.events.lock().map(|e| e.clone()).unwrap_or_default()
    }
}

// ---------------------------------------------------------------- calendario

pub fn calendar_access() -> Access {
    #[cfg(target_os = "macos")]
    {
        juanola_calendar::eventkit::access()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Access::Unsupported
    }
}

pub fn request_calendar_access() -> Access {
    #[cfg(target_os = "macos")]
    {
        juanola_calendar::eventkit::request_access(Duration::from_secs(120))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Access::Unsupported
    }
}

/// Eventos de calendario de `[from, to)` (vacío sin permiso).
pub fn calendar_events(from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<CalendarEvent> {
    #[cfg(target_os = "macos")]
    {
        juanola_calendar::eventkit::events(from, to)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (from, to);
        Vec::new()
    }
}

/// Relee el calendario (desde hace 1 h hasta dentro de 30 días) y lo guarda
/// en caché. 30 días para que «Coming up» enseñe la siguiente reunión aunque
/// no sea esta semana.
pub fn refresh_events(detection: &Detection) -> Vec<CalendarEvent> {
    let now = Utc::now();
    let events = calendar_events(now - chrono::Duration::hours(1), now + chrono::Duration::days(30));
    if let Ok(mut cache) = detection.events.lock() {
        *cache = events.clone();
    }
    events
}

// ------------------------------------------------------------------- ventana

/// Muestra (o actualiza) la ventana de aviso arriba a la derecha.
pub fn show(app: &AppHandle, n: Notification) {
    let state = app.state::<AppState>();
    if let Ok(mut cur) = state.detection.current.lock() {
        *cur = Some(n.clone());
    }
    tracing::info!(kind = ?n.kind, title = %n.title, body = %n.body, "aviso de reunión");
    if let Some(w) = app.get_webview_window(WINDOW) {
        let _ = w.emit("notification://update", &n);
        let _ = w.show();
        return;
    }
    let (x, y) = match app.primary_monitor() {
        Ok(Some(m)) => {
            let scale = m.scale_factor();
            let area = m.work_area();
            let right = (area.position.x as f64 + area.size.width as f64) / scale;
            (right - WIDTH - 8.0, area.position.y as f64 / scale + 8.0)
        }
        _ => (100.0, 40.0),
    };
    let built = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App("index.html#/notification".into()))
        .title("Juanola")
        .inner_size(WIDTH, HEIGHT)
        .position(x, y)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        // Que no robe el teclado a la app de la llamada, y que el primer clic
        // llegue a «Take Notes» aunque Juanola no esté activa.
        .focused(false)
        .focusable(false)
        .accept_first_mouse(true)
        .build();
    match built {
        Ok(w) => {
            let _ = w.set_size(LogicalSize::new(WIDTH, HEIGHT));
            let _ = w.set_position(LogicalPosition::new(x, y));
            let main_focused = app.get_webview_window("main").and_then(|m| m.is_focused().ok());
            tracing::info!(
                pos = ?w.outer_position().ok(),
                size = ?w.outer_size().ok(),
                monitor = ?app.primary_monitor().ok().flatten().map(|m| (*m.size(), m.scale_factor())),
                visible = ?w.is_visible().ok(),
                focused = ?w.is_focused().ok(),
                main_focused = ?main_focused,
                "ventana de aviso abierta"
            );
        }
        Err(e) => tracing::warn!(error = %e, "no se pudo abrir el aviso"),
    }
}

pub fn close(app: &AppHandle) {
    app.state::<AppState>().detection.take_current();
    if let Some(w) = app.get_webview_window(WINDOW) {
        let _ = w.close();
    }
}

/// Trae la ventana principal al frente.
pub fn focus_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn adhoc_notification(app: DetectedApp, event: Option<&CalendarEvent>) -> Notification {
    let title = notification_title(&app.kind).to_string();
    let body = match event {
        Some(e) => format!("{} · {}", app.name, e.title),
        None => app.name.clone(),
    };
    Notification { kind: NotificationKind::Adhoc, title, body, app: Some(app), event: event.cloned() }
}

pub fn calendar_notification(e: &CalendarEvent, now: DateTime<Utc>) -> Notification {
    let mins = ((e.start - now).num_seconds().max(0) + 59) / 60;
    let body = if mins <= 0 { "Starting now".to_string() } else { format!("Starts in {mins} min") };
    Notification { kind: NotificationKind::Calendar, title: e.title.clone(), body, app: None, event: Some(e.clone()) }
}

// --------------------------------------------------------------------- hilo

/// Arranca el hilo de sondeo. Vive lo que la app.
pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("juanola-detect".into())
        .spawn(move || run(app))
        .expect("hilo de detección");
}

fn run(app: AppHandle) {
    let t0 = Instant::now();
    let own_pid = std::process::id() as i32;
    let mut watcher = MicWatcher::new(WatcherConfig::default());
    let mut last_calendar: Option<Instant> = None;
    loop {
        let state = app.state::<AppState>();
        let det = &state.detection;
        let recording = state.recording.lock().map(|r| r.is_some()).unwrap_or(false);

        let users = juanola_detect::mic_users(own_pid);
        for ev in watcher.update(t0.elapsed(), &users) {
            match ev {
                DetectEvent::MicInUse { app: detected } => {
                    tracing::info!(
                        app = %detected.name,
                        bundle = %detected.bundle_id,
                        recording,
                        "micrófono en uso"
                    );
                    if recording || !det.meetings.load(Ordering::Relaxed) {
                        continue;
                    }
                    let events = det.cached_events();
                    let event = juanola_calendar::event_for_call(&events, Utc::now());
                    show(&app, adhoc_notification(detected, event));
                }
                DetectEvent::MicReleased { app: detected } => {
                    tracing::info!(app = %detected.name, "micrófono liberado");
                    let showing = det
                        .current()
                        .is_some_and(|n| n.app.as_ref().is_some_and(|a| a.bundle_id == detected.bundle_id));
                    if showing {
                        close(&app);
                    }
                    let origin = det.origin_app.lock().ok().and_then(|o| o.clone());
                    if recording && origin.as_deref() == Some(detected.bundle_id.as_str()) {
                        tracing::info!(app = %detected.name, "la llamada ha terminado: se para la grabación");
                        let _ = app.emit_to("main", "detect://meeting-ended", &detected);
                    }
                }
            }
        }

        if last_calendar.is_none_or(|t| t.elapsed() >= Duration::from_secs(30)) {
            last_calendar = Some(Instant::now());
            if calendar_access() == Access::Granted {
                let events = refresh_events(det);
                let now = Utc::now();
                if det.reminders.load(Ordering::Relaxed) && !recording {
                    for e in juanola_calendar::due_reminders(&events, now) {
                        let fresh = det.reminded.lock().map(|mut r| r.insert(e.id.clone())).unwrap_or(false);
                        if fresh {
                            show(&app, calendar_notification(e, now));
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
