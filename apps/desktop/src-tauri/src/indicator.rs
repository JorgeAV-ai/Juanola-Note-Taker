//! Icono de la barra de menús e indicador flotante de grabación.
//!
//! - **Barra de menús**: el rombo de Juanola (hueco en reposo, relleno
//!   mientras graba) con el tiempo de la sesión al lado y un menú para abrir
//!   la app, grabar una nota nueva, ir a la nota, pausar, reanudar o parar.
//! - **Indicador flotante** (`live-indicator`): una píldora pequeña arriba en
//!   el centro con el rombo, el tiempo, el nivel y los controles. Solo se ve
//!   mientras se graba y la ventana principal no tiene el foco; es un panel
//!   que no activa la app, como el aviso de reunión.
//!
//! El estado lo alimenta `recording.rs` con [`on_state`]; un hilo refresca
//! el tiempo una vez por segundo mientras hay sesión.
//!
//! Parar no se hace aquí: se pide a la ventana principal con
//! `recording://stop-requested`, que para igual que el botón de la nota
//! (y lanza el resumen).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use juanola_core_types::Id;
use serde::{Deserialize, Serialize};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

pub const WINDOW: &str = "live-indicator";
const TRAY: &str = "juanola";
/// Lo que mide la píldora; esquinas y sombra las pone AppKit.
const WIDTH: f64 = 232.0;
const HEIGHT: f64 = 44.0;
/// Separación bajo la barra de menús.
const INSET: f64 = 10.0;

/// Textos del menú en el idioma de la interfaz. Los manda el frontend al
/// arrancar y al cambiar de idioma; hasta entonces, en inglés.
#[derive(Debug, Clone, Deserialize)]
pub struct Labels {
    pub open: String,
    pub new_note: String,
    pub go_to_note: String,
    pub pause: String,
    pub resume: String,
    pub stop: String,
    pub recording: String,
    pub paused: String,
    pub quit: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            open: "Open Juanola".into(),
            new_note: "Record a new note".into(),
            go_to_note: "Go to the note".into(),
            pause: "Pause".into(),
            resume: "Resume".into(),
            stop: "Stop recording".into(),
            recording: "Recording".into(),
            paused: "Paused".into(),
            quit: "Quit Juanola".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Starting,
    Recording,
    Paused,
}

/// Sesión en curso, tal como la ven el menú y la píldora.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub note_id: Id,
    pub phase: Phase,
    /// Segundos grabados, sin contar las pausas.
    pub secs: u64,
}

#[derive(Default)]
struct Session {
    note_id: Option<Id>,
    phase: Option<Phase>,
    /// Desde cuándo corre el reloj (None en pausa o arrancando).
    since: Option<Instant>,
    /// Lo grabado antes de la última pausa.
    acc: Duration,
}

impl Session {
    fn snapshot(&self) -> Option<Snapshot> {
        let secs = (self.acc + self.since.map_or(Duration::ZERO, |t| t.elapsed())).as_secs();
        Some(Snapshot { note_id: self.note_id?, phase: self.phase?, secs })
    }
}

#[derive(Default)]
pub struct Indicator {
    session: Mutex<Session>,
    labels: Mutex<Labels>,
    /// La ventana principal tiene el foco: la píldora sobra.
    main_focused: Mutex<bool>,
    /// Lo último que se pintó en el menú, para no rehacerlo cada segundo.
    menu_key: Mutex<Option<(Option<Phase>, bool)>>,
}

impl Indicator {
    pub fn new() -> Self {
        Self { main_focused: Mutex::new(true), ..Default::default() }
    }

    pub fn snapshot(&self) -> Option<Snapshot> {
        self.session.lock().ok()?.snapshot()
    }
}

/// Crea el icono de la barra de menús y el hilo que refresca el tiempo.
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app, None)?;
    TrayIconBuilder::with_id(TRAY)
        .icon(diamond(false))
        .icon_as_template(true)
        .tooltip("Juanola")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, ev| on_menu(app, ev.id.as_ref()))
        .build(app)?;
    let app = app.clone();
    std::thread::Builder::new()
        .name("juanola-indicator".into())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
                tick(&app);
            }
        })
        .map_err(|e| tauri::Error::Anyhow(e.into()))?;
    Ok(())
}

/// Cambio de estado de la grabación (lo llama `recording.rs` con cada
/// `recording://state`).
pub fn on_state(app: &AppHandle, note_id: Id, state: &str) {
    let ind = app.state::<crate::state::AppState>().indicator.clone();
    {
        let Ok(mut s) = ind.session.lock() else { return };
        match state {
            "starting" => *s = Session { note_id: Some(note_id), phase: Some(Phase::Starting), ..Default::default() },
            "recording" => {
                s.note_id = Some(note_id);
                s.phase = Some(Phase::Recording);
                s.since.get_or_insert_with(Instant::now);
            }
            "paused" => {
                if let Some(t) = s.since.take() {
                    s.acc += t.elapsed();
                }
                s.phase = Some(Phase::Paused);
            }
            // «finishing», «stopped» o «error»: la sesión ya no graba.
            "finishing" | "stopped" | "error" => {
                if s.note_id == Some(note_id) {
                    *s = Session::default();
                }
            }
            _ => return,
        }
    }
    refresh(app);
}

/// La ventana principal ganó o perdió el foco.
pub fn on_main_focus(app: &AppHandle, focused: bool) {
    let ind = app.state::<crate::state::AppState>().indicator.clone();
    if let Ok(mut f) = ind.main_focused.lock() {
        *f = focused;
    }
    refresh(app);
}

pub fn set_labels(app: &AppHandle, labels: Labels) {
    let ind = app.state::<crate::state::AppState>().indicator.clone();
    if let Ok(mut l) = ind.labels.lock() {
        *l = labels;
    }
    if let Ok(mut k) = ind.menu_key.lock() {
        *k = None;
    }
    refresh(app);
}

fn tick(app: &AppHandle) {
    let Some(snap) = app.state::<crate::state::AppState>().indicator.snapshot() else { return };
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_title(Some(clock(snap.secs)));
    }
    let _ = app.emit_to(WINDOW, "indicator://update", Some(&snap));
}

/// Pone al día icono, título, menú y píldora con el estado actual.
fn refresh(app: &AppHandle) {
    let ind = app.state::<crate::state::AppState>().indicator.clone();
    let snap = ind.snapshot();
    let focused = ind.main_focused.lock().map(|f| *f).unwrap_or(true);
    if let Some(tray) = app.tray_by_id(TRAY) {
        update_tray(app, &tray, &ind, snap.as_ref());
    }
    let _ = app.emit_to(WINDOW, "indicator://update", snap.as_ref());
    if snap.is_some() && !focused {
        show_pill(app);
    } else {
        hide_pill(app);
    }
}

fn update_tray(app: &AppHandle, tray: &TrayIcon, ind: &Indicator, snap: Option<&Snapshot>) {
    let phase = snap.map(|s| s.phase);
    let key = (phase, true);
    let changed = ind.menu_key.lock().map(|mut k| k.replace(key) != Some(key)).unwrap_or(true);
    if !changed {
        return;
    }
    let _ = tray.set_icon(Some(diamond(snap.is_some())));
    let _ = tray.set_icon_as_template(true);
    let _ = tray.set_title(snap.map(|s| clock(s.secs)));
    match build_menu(app, phase) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(e) => tracing::warn!(error = %e, "menú de la barra"),
    }
}

fn build_menu(app: &AppHandle, phase: Option<Phase>) -> tauri::Result<Menu<tauri::Wry>> {
    let l = app.state::<crate::state::AppState>().indicator.labels.lock().map(|l| l.clone()).unwrap_or_default();
    let item = |id: &str, text: &str, enabled: bool| MenuItem::with_id(app, id, text, enabled, None::<&str>);
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = item("quit", &l.quit, true)?;
    let open = item("open", &l.open, true)?;
    match phase {
        None => Menu::with_items(app, &[&open, &item("new_note", &l.new_note, true)?, &sep, &quit]),
        Some(p) => {
            let status = item("status", if p == Phase::Paused { &l.paused } else { &l.recording }, false)?;
            let toggle =
                if p == Phase::Paused { item("resume", &l.resume, true)? } else { item("pause", &l.pause, p == Phase::Recording)? };
            let sep2 = PredefinedMenuItem::separator(app)?;
            Menu::with_items(
                app,
                &[&status, &item("go_to_note", &l.go_to_note, true)?, &toggle, &item("stop", &l.stop, true)?, &sep, &open, &sep2, &quit],
            )
        }
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => crate::detect::focus_main(app),
        "new_note" => {
            crate::detect::focus_main(app);
            let _ = app.emit_to("main", "tray://new-note", ());
        }
        "go_to_note" => open_note(app),
        "pause" => pause(app, true),
        "resume" => pause(app, false),
        "stop" => request_stop(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Trae la ventana principal y le pide que abra la nota que se graba.
pub fn open_note(app: &AppHandle) {
    let Some(snap) = app.state::<crate::state::AppState>().indicator.snapshot() else { return };
    crate::detect::focus_main(app);
    let _ = app.emit_to("main", "indicator://open-note", NoteRef { note_id: snap.note_id });
}

pub fn pause(app: &AppHandle, pause: bool) {
    let state = app.state::<crate::state::AppState>();
    let Ok(slot) = state.recording.lock() else { return };
    match slot.as_ref() {
        Some(a) if pause => a.pause(),
        Some(a) => a.resume(),
        None => {}
    }
}

/// Pide a la ventana principal que pare (como el botón de la nota, que
/// además lanza el resumen) y la trae para que se vea.
pub fn request_stop(app: &AppHandle) {
    let Some(snap) = app.state::<crate::state::AppState>().indicator.snapshot() else { return };
    crate::detect::focus_main(app);
    let _ = app.emit_to("main", "recording://stop-requested", NoteRef { note_id: snap.note_id });
}

#[derive(Serialize, Clone)]
struct NoteRef {
    note_id: Id,
}

// ------------------------------------------------------------------ píldora

fn show_pill(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(WINDOW) {
        #[cfg(target_os = "macos")]
        crate::detect::order_front(&w);
        #[cfg(not(target_os = "macos"))]
        let _ = w.show();
        return;
    }
    let (x, y) = match app.primary_monitor() {
        Ok(Some(m)) => {
            let scale = m.scale_factor();
            let area = m.work_area();
            let left = area.position.x as f64 / scale;
            let width = area.size.width as f64 / scale;
            (left + (width - WIDTH) / 2.0, area.position.y as f64 / scale + INSET)
        }
        _ => (400.0, 40.0),
    };
    // Mismo montaje que el aviso de reunión (ver `detect::show`): sin robar
    // el foco ni activar la app, y visible sobre apps a pantalla completa.
    let built = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App("index.html#/indicator".into()))
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
        .focused(false)
        .focusable(false)
        .accept_first_mouse(true)
        .visible(cfg!(not(target_os = "macos")))
        .build();
    match built {
        Ok(w) => {
            #[cfg(target_os = "macos")]
            crate::detect::float_over_fullscreen(&w, HEIGHT / 2.0);
            let _ = w.set_position(LogicalPosition::new(x, y));
            tracing::info!("indicador de grabación abierto");
        }
        Err(e) => tracing::warn!(error = %e, "no se pudo abrir el indicador"),
    }
}

/// Oculta la píldora sin cerrarla: se reutiliza en la siguiente salida.
fn hide_pill(app: &AppHandle) {
    let Some(w) = app.get_webview_window(WINDOW) else { return };
    #[cfg(target_os = "macos")]
    crate::detect::order_out(&w);
    #[cfg(not(target_os = "macos"))]
    let _ = w.hide();
}

// -------------------------------------------------------------------- icono

/// `m:ss` hasta la hora y `h:mm:ss` después.
pub fn clock(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// El rombo del logo como icono de plantilla (negro sobre transparente; macOS
/// lo tiñe). Relleno mientras graba, solo el contorno en reposo. Se dibuja
/// aquí, con supermuestreo, para no llevar otro PNG.
pub fn diamond(filled: bool) -> Image<'static> {
    const H: u32 = 36; // 18 pt a 2x
    const W: u32 = 26;
    const SS: u32 = 4;
    // Vértices del logo (viewBox 64×100), ajustados al lienzo con margen.
    let pts = [(32.0, 5.0), (58.0, 49.0), (33.0, 95.0), (6.0, 51.0)];
    let (sx, sy) = ((W as f64 - 2.0) / 64.0, (H as f64 - 3.0) / 100.0);
    let poly: Vec<(f64, f64)> = pts.iter().map(|(x, y)| (1.0 + x * sx, 1.5 + y * sy)).collect();
    // Contorno: distancia al borde menor que el grosor.
    let stroke = 2.4;
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for py in 0..H {
        for px in 0..W {
            let mut hits = 0;
            for j in 0..SS {
                for i in 0..SS {
                    let x = px as f64 + (i as f64 + 0.5) / SS as f64;
                    let y = py as f64 + (j as f64 + 0.5) / SS as f64;
                    if inside(&poly, x, y) && (filled || edge_distance(&poly, x, y) < stroke) {
                        hits += 1;
                    }
                }
            }
            let a = (hits * 255 / (SS * SS)) as u8;
            rgba[((py * W + px) * 4 + 3) as usize] = a;
        }
    }
    Image::new_owned(rgba, W, H)
}

fn inside(poly: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[(i + n - 1) % n];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            c = !c;
        }
    }
    c
}

fn edge_distance(poly: &[(f64, f64)], x: f64, y: f64) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (ax, ay) = poly[i];
            let (bx, by) = poly[(i + 1) % n];
            let (dx, dy) = (bx - ax, by - ay);
            let t = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            ((x - ax - t * dx).powi(2) + (y - ay - t * dy).powi(2)).sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_format() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(75), "1:15");
        assert_eq!(clock(3600 + 62), "1:01:02");
    }

    #[test]
    fn diamond_has_shape() {
        let alpha = |img: &Image| img.rgba().chunks(4).filter(|p| p[3] > 128).count();
        let (full, outline) = (diamond(true), diamond(false));
        assert_eq!((full.width(), full.height()), (26, 36));
        // Relleno ocupa más que el contorno, y ambos algo.
        assert!(alpha(&outline) > 50);
        assert!(alpha(&full) > alpha(&outline) + 100);
        // Las esquinas del lienzo quedan transparentes.
        assert_eq!(full.rgba()[3], 0);
    }

    #[test]
    fn session_clock_skips_pauses() {
        let mut s = Session { note_id: Some(Id::nil()), phase: Some(Phase::Paused), since: None, acc: Duration::from_secs(90) };
        assert_eq!(s.snapshot().unwrap().secs, 90);
        s.since = Some(Instant::now() - Duration::from_secs(10));
        assert_eq!(s.snapshot().unwrap().secs, 100);
        assert!(Session::default().snapshot().is_none());
    }
}
