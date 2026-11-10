//! Detección de reuniones y avisos, como Granola:
//!
//! - **Ad hoc**: otra app abre el micrófono (Zoom, Teams, Meet en el
//!   navegador…) → «Meeting detected» con «Take Notes». Si hay un evento del
//!   calendario en curso, la nota se asocia a él.
//! - **Calendario**: 1 minuto antes de un evento con 2+ asistentes.
//! - **Fin de la llamada**: si la grabación empezó desde un aviso y la app de
//!   la llamada suelta el micrófono durante [`END_WAIT`], se avisa al frontend
//!   para que pare. Con un aviso de calendario la app se descubre después: la
//!   primera app de reuniones que tenga el micrófono mientras se graba.
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
use juanola_detect::{holds_mic, notification_title, DetectEvent, DetectedApp, EndTracker, MicWatcher, WatcherConfig};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::state::AppState;

pub const WINDOW: &str = "notification";
/// La ventana mide lo mismo que la tarjeta: sin margen transparente, porque
/// macOS pintaba un velo blanco en él. Esquinas y sombra las pone AppKit.
const WIDTH: f64 = 368.0;
const HEIGHT: f64 = 110.0;
const RADIUS: f64 = 16.0;
/// Separación del borde de la pantalla.
const INSET: f64 = 16.0;

/// Tiempo sin micrófono para dar por terminada la llamada que se graba.
const END_WAIT: Duration = Duration::from_secs(15);
/// Un aviso de calendario sin atender se cierra este tiempo después de empezar.
const CALENDAR_NOTICE_TTL: chrono::Duration = chrono::Duration::minutes(10);
/// El sondeo solo necesita los eventos de alrededor de ahora (avisos y
/// asociar llamadas); «Coming up» pide su propio rango.
const POLL_PAST: chrono::Duration = chrono::Duration::hours(1);
const POLL_AHEAD: chrono::Duration = chrono::Duration::hours(2);

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

/// De qué llamada es la grabación en curso (para pararla cuando termine).
#[derive(Debug, Clone, PartialEq)]
pub enum Origin {
    /// Grabación manual (o ninguna): no se para sola.
    None,
    /// Salió de un aviso de calendario: la app se fija en cuanto una app de
    /// reuniones tenga el micrófono.
    Pending,
    /// Bundle id de la app de la llamada.
    App(String),
}

/// `detect://meeting-ended`: la nota cuya grabación hay que parar.
#[derive(Debug, Clone, Serialize)]
pub struct MeetingEnded {
    pub note_id: juanola_core_types::Id,
    pub app: DetectedApp,
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
    /// Llamada que se está grabando (si la grabación salió de un aviso).
    origin: Mutex<Origin>,
    /// Eventos del calendario de alrededor de ahora.
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
            origin: Mutex::new(Origin::None),
            events: Mutex::new(Vec::new()),
            reminded: Mutex::new(HashSet::new()),
        }
    }

    pub fn settings(&self) -> DetectionSettings {
        DetectionSettings { meetings: self.meetings.load(Ordering::Relaxed), calendar_reminders: self.reminders.load(Ordering::Relaxed) }
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

    pub fn set_origin(&self, origin: Origin) {
        if let Ok(mut o) = self.origin.lock() {
            *o = origin;
        }
    }

    fn origin(&self) -> Origin {
        self.origin.lock().map(|o| o.clone()).unwrap_or(Origin::None)
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

/// Relee el calendario alrededor de ahora y lo guarda en caché para asociar
/// llamadas. También olvida los recordatorios de eventos que ya quedaron atrás.
fn refresh_events(detection: &Detection) -> Vec<CalendarEvent> {
    let now = Utc::now();
    let events = calendar_events(now - POLL_PAST, now + POLL_AHEAD);
    if let Ok(mut cache) = detection.events.lock() {
        *cache = events.clone();
    }
    if let Ok(mut r) = detection.reminded.lock() {
        r.retain(|id| events.iter().any(|e| &e.id == id));
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
        #[cfg(target_os = "macos")]
        raise(app);
        #[cfg(not(target_os = "macos"))]
        let _ = w.show();
        return;
    }
    let (x, y) = match app.primary_monitor() {
        Ok(Some(m)) => {
            let scale = m.scale_factor();
            let area = m.work_area();
            let right = (area.position.x as f64 + area.size.width as f64) / scale;
            (right - WIDTH - INSET, area.position.y as f64 / scale + INSET)
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
        // En macOS se crea oculta y se enseña con `orderFrontRegardless`: el
        // `show` de Tauri activa la app y saca al usuario de la reunión si
        // está a pantalla completa.
        .visible(cfg!(not(target_os = "macos")))
        .build();
    match built {
        Ok(w) => {
            #[cfg(target_os = "macos")]
            float_over_fullscreen(&w);
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

/// Las reuniones suelen ir a pantalla completa, y una ventana «siempre
/// encima» normal no entra en el espacio de otra app a pantalla completa.
/// Probado (8 nov, Chrome a pantalla completa): hace falta que sea un
/// `NSPanel` no activante con `FullScreenAuxiliary`; con solo el
/// comportamiento, o subiendo el nivel, no aparece. Y se enseña con
/// `orderFrontRegardless`, que no activa Juanola ni cambia de espacio.
///
/// Ojo: la ventana es de la clase `TaoWindow` de tao, que tiene estado propio.
/// Mientras es panel, tao no la toca; para cerrarla (o al salir de la app) hay
/// que devolverle su clase, o tao lanza una excepción de Objective-C y la app
/// aborta (visto al cerrar el aviso). Ver [`restore_class`].
/// AppKit solo en el hilo principal.
#[cfg(target_os = "macos")]
static TAO_CLASS: std::sync::atomic::AtomicPtr<objc2::runtime::AnyClass> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[cfg(target_os = "macos")]
fn float_over_fullscreen(w: &tauri::WebviewWindow) {
    let win = w.clone();
    let _ = w.run_on_main_thread(move || {
        let Ok(ptr) = win.ns_window() else { return };
        use objc2::ClassType;
        use objc2_app_kit::{NSPanel, NSWindow, NSWindowCollectionBehavior as B, NSWindowStyleMask};
        // SAFETY: `ptr` es la NSWindow viva de esta ventana y estamos en el
        // hilo principal. NSPanel es subclase de NSWindow sin ivars propias
        // (lo mismo que hace tauri-nspanel); la ventana no puede ser clave,
        // así que no se pierde nada de lo que tao añade a su clase.
        unsafe {
            let obj = ptr as *mut objc2::runtime::AnyObject;
            let original = objc2::ffi::object_getClass(obj);
            TAO_CLASS.store(original as *mut _, std::sync::atomic::Ordering::SeqCst);
            objc2::ffi::object_setClass(obj, NSPanel::class());
        }
        let ns: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
        ns.setStyleMask(ns.styleMask() | NSWindowStyleMask::NonactivatingPanel);
        ns.setHidesOnDeactivate(false);
        ns.setCollectionBehavior(B::CanJoinAllSpaces | B::FullScreenAuxiliary | B::Stationary | B::IgnoresCycle);
        // Esquinas redondeadas recortando la propia vista (la página entera,
        // velo incluido) y sombra nativa, que sigue esa forma.
        if let Some(view) = ns.contentView() {
            view.setWantsLayer(true);
            if let Some(layer) = view.layer() {
                layer.setCornerRadius(RADIUS);
                layer.setMasksToBounds(true);
            }
        }
        ns.setHasShadow(true);
        ns.orderFrontRegardless();
    });
    // La sombra se calcula con lo pintado: se recalcula cuando la página ya
    // ha cargado.
    let win = w.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        let w2 = win.clone();
        let _ = win.run_on_main_thread(move || {
            if let Ok(ptr) = w2.ns_window() {
                // SAFETY: NSWindow viva, hilo principal.
                unsafe { &*(ptr as *const objc2_app_kit::NSWindow) }.invalidateShadow();
            }
        });
    });
}

/// Trae al frente el aviso ya abierto (al actualizarlo), sin activar la app.
#[cfg(target_os = "macos")]
fn raise(app: &AppHandle) {
    let Some(w) = app.get_webview_window(WINDOW) else { return };
    let win = w.clone();
    let _ = w.run_on_main_thread(move || {
        let Ok(ptr) = win.ns_window() else { return };
        // SAFETY: NSWindow viva, hilo principal.
        let ns: &objc2_app_kit::NSWindow = unsafe { &*(ptr as *const objc2_app_kit::NSWindow) };
        ns.orderFrontRegardless();
    });
}

/// Devuelve al aviso su clase de tao (ver [`float_over_fullscreen`]). Solo
/// en el hilo principal.
#[cfg(target_os = "macos")]
pub fn restore_class(w: &tauri::WebviewWindow) {
    let class = TAO_CLASS.load(std::sync::atomic::Ordering::SeqCst);
    let Ok(ptr) = w.ns_window() else { return };
    if class.is_null() || ptr.is_null() {
        return;
    }
    // SAFETY: es la clase con la que tao creó esta ventana.
    unsafe { objc2::ffi::object_setClass(ptr as *mut objc2::runtime::AnyObject, class) };
}

/// Cierra el aviso y devuelve el que se mostraba (si había).
pub fn close(app: &AppHandle) -> Option<Notification> {
    let n = app.state::<AppState>().detection.take_current();
    if let Some(w) = app.get_webview_window(WINDOW) {
        #[cfg(target_os = "macos")]
        {
            // Clase y cierre en el mismo paso por el hilo principal, en orden.
            let win = w.clone();
            let _ = w.run_on_main_thread(move || {
                restore_class(&win);
                let _ = win.close();
            });
        }
        #[cfg(not(target_os = "macos"))]
        let _ = w.close();
    }
    n
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
    let secs = (e.start - now).num_seconds();
    let body = if secs > 0 {
        format!("Starts in {} min", (secs + 59) / 60)
    } else if secs > -60 {
        "Starting now".to_string()
    } else {
        format!("Started {} min ago", -secs / 60)
    };
    Notification { kind: NotificationKind::Calendar, title: e.title.clone(), body, app: None, event: Some(e.clone()) }
}

/// Cierra el aviso si es del tipo que se acaba de desactivar en Ajustes.
pub fn close_disabled(app: &AppHandle, s: DetectionSettings) {
    let kind = app.state::<AppState>().detection.current().map(|n| n.kind);
    let off = match kind {
        Some(NotificationKind::Adhoc) => !s.meetings,
        Some(NotificationKind::Calendar) => !s.calendar_reminders,
        None => false,
    };
    if off {
        close(app);
    }
}

// --------------------------------------------------------------------- hilo

/// Arranca el hilo de sondeo. Vive lo que la app.
pub fn spawn(app: AppHandle) {
    std::thread::Builder::new().name("juanola-detect".into()).spawn(move || run(app)).expect("hilo de detección");
}

fn run(app: AppHandle) {
    let t0 = Instant::now();
    let own_pid = std::process::id() as i32;
    let mut watcher = MicWatcher::new(WatcherConfig::default());
    let mut end = EndTracker::new(END_WAIT);
    // Segundos con un origen puesto pero sin grabación (p. ej. el arranque
    // automático falló): pasado un rato se olvida, para no parar después una
    // grabación manual.
    let mut orphan_secs = 0u32;
    let mut last_calendar: Option<Instant> = None;
    loop {
        let state = app.state::<AppState>();
        let det = &state.detection;
        let recording = state.recording.lock().ok().and_then(|r| r.as_ref().map(|a| a.note_id));
        let now = t0.elapsed();

        let users = juanola_detect::mic_users(own_pid);
        for ev in watcher.update(now, &users) {
            match ev {
                DetectEvent::MicInUse { app: detected } => {
                    tracing::info!(
                        app = %detected.name,
                        bundle = %detected.bundle_id,
                        recording = recording.is_some(),
                        "micrófono en uso"
                    );
                    if recording.is_some() || !det.meetings.load(Ordering::Relaxed) {
                        continue;
                    }
                    let events = det.cached_events();
                    let event = juanola_calendar::event_for_call(&events, Utc::now());
                    show(&app, adhoc_notification(detected, event));
                }
                DetectEvent::MicReleased { app: detected } => {
                    tracing::info!(app = %detected.name, "micrófono liberado");
                    let showing = det.current().is_some_and(|n| n.app.as_ref().is_some_and(|a| a.bundle_id == detected.bundle_id));
                    if showing {
                        close(&app);
                    }
                }
            }
        }

        match (recording, det.origin()) {
            (Some(note_id), Origin::App(bundle)) => {
                if end.update(now, holds_mic(&users, &bundle)) {
                    let detected = juanola_detect::classify(&bundle);
                    tracing::info!(app = %bundle, "la llamada ha terminado: se para la grabación");
                    det.set_origin(Origin::None);
                    if let Some(detected) = detected {
                        let _ = app.emit_to("main", "detect://meeting-ended", MeetingEnded { note_id, app: detected });
                    }
                }
            }
            (Some(_), Origin::Pending) => {
                // Grabación desde un aviso de calendario: la llamada es la
                // primera app de reuniones que tenga el micrófono.
                if let Some(a) = watcher.active().next() {
                    tracing::info!(app = %a.name, "grabación asociada a la llamada");
                    det.set_origin(Origin::App(a.bundle_id.clone()));
                }
                end.reset();
            }
            (None, Origin::None) => {
                orphan_secs = 0;
                end.reset();
            }
            (None, _) => {
                orphan_secs += 1;
                if orphan_secs >= 30 {
                    det.set_origin(Origin::None);
                }
                end.reset();
            }
            _ => end.reset(),
        }
        if recording.is_some() {
            orphan_secs = 0;
        }

        // Ya se graba (desde el aviso o a mano): el aviso sobra.
        if recording.is_some() && det.current().is_some() {
            close(&app);
        }

        // Aviso de calendario: cuenta atrás y caducidad.
        if let Some(n) = det.current().filter(|n| n.kind == NotificationKind::Calendar) {
            if let Some(e) = &n.event {
                let at = Utc::now();
                if at > e.start + CALENDAR_NOTICE_TTL {
                    close(&app);
                } else {
                    let fresh = calendar_notification(e, at);
                    if fresh.body != n.body {
                        show(&app, fresh);
                    }
                }
            }
        }

        if last_calendar.is_none_or(|t| t.elapsed() >= Duration::from_secs(30)) {
            last_calendar = Some(Instant::now());
            if calendar_access() == Access::Granted {
                let events = refresh_events(det);
                let at = Utc::now();
                if det.reminders.load(Ordering::Relaxed) && recording.is_none() {
                    for e in juanola_calendar::due_reminders(&events, at) {
                        let fresh = det.reminded.lock().map(|mut r| r.insert(e.id.clone())).unwrap_or(false);
                        if fresh {
                            show(&app, calendar_notification(e, at));
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
