//! Detección de reuniones: qué app está usando el micrófono, a qué app de
//! videollamada corresponde y cuándo avisar. Produce los títulos exactos de
//! Granola para las notificaciones.
//!
//! - [`mic_users`] pregunta al sistema qué procesos tienen la entrada de audio
//!   abierta (en macOS 14.2+, objetos de proceso de Core Audio; no pide permisos).
//! - [`classify`] traduce el bundle id a una app conocida, o lo descarta (Siri,
//!   dictado y demás servicios del sistema que escuchan siempre).
//! - [`MicWatcher`] filtra parpadeos: avisa cuando una app lleva un rato con el
//!   micrófono y da la llamada por terminada cuando lo suelta.

use std::collections::HashMap;
use std::time::Duration;

#[cfg(target_os = "macos")]
mod macos;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingApp {
    Zoom,
    Teams,
    GoogleMeet,
    SlackHuddle,
    FaceTime,
    WhatsApp,
    /// Navegador: puede ser Meet, Teams web, Jitsi…; no se sabe sin leer la pestaña.
    Browser,
    Other,
}

/// Proceso con la entrada de audio abierta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicUser {
    pub pid: i32,
    pub bundle_id: String,
}

/// App reconocida detrás de un [`MicUser`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DetectedApp {
    /// Bundle id de la app principal (sin el sufijo de sus procesos auxiliares).
    pub bundle_id: String,
    /// Nombre para la notificación («Zoom», «Google Chrome»…).
    pub name: String,
    pub kind: MeetingApp,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DetectEvent {
    /// Una app lleva [`WatcherConfig::min_on`] con el micrófono abierto.
    MicInUse { app: DetectedApp },
    /// La app soltó el micrófono hace [`WatcherConfig::grace`].
    MicReleased { app: DetectedApp },
}

/// Título de la notificación ad hoc, igual que en Granola.
pub fn notification_title(app: &MeetingApp) -> &'static str {
    match app {
        MeetingApp::SlackHuddle => "Huddle detected",
        MeetingApp::FaceTime | MeetingApp::WhatsApp => "Call detected",
        _ => "Meeting detected",
    }
}

/// Procesos que tienen ahora mismo la entrada de audio abierta, salvo `own_pid`.
/// Vacío en sistemas sin soporte (macOS < 14.2, otros SO por ahora).
pub fn mic_users(own_pid: i32) -> Vec<MicUser> {
    #[cfg(target_os = "macos")]
    {
        macos::mic_users().into_iter().filter(|u| u.pid != own_pid).collect()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = own_pid;
        Vec::new()
    }
}

/// Proceso cliente de Core Audio (diagnóstico, ver el ejemplo `mic_users --all`).
#[derive(Debug, Clone)]
pub struct AudioProcess {
    pub pid: i32,
    pub bundle_id: String,
    pub running: bool,
    pub input: bool,
    pub output: bool,
    /// Nº de dispositivos de entrada que usa.
    pub input_devices: usize,
}

#[doc(hidden)]
pub fn describe_devices() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        macos::describe_devices()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

#[doc(hidden)]
pub fn process_devices(pid: i32) -> (Vec<u32>, Vec<u32>) {
    #[cfg(target_os = "macos")]
    {
        macos::process_devices(pid)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        (Vec::new(), Vec::new())
    }
}

/// Si el micrófono predeterminado está en uso por algún proceso.
pub fn default_input_running() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::default_input_running()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn audio_processes() -> Vec<AudioProcess> {
    #[cfg(target_os = "macos")]
    {
        macos::audio_processes()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

/// Servicios del sistema que abren el micrófono sin que haya una llamada.
const IGNORED_PREFIXES: &[&str] = &[
    "com.apple.corespeechd",
    "com.apple.CoreSpeech",
    "com.apple.siri",
    "com.apple.Siri",
    "com.apple.assistant",
    "com.apple.SpeechRecognitionCore",
    "com.apple.speech",
    "com.apple.dictation",
    "com.apple.DictationIM",
    "com.apple.accessibility",
    "com.apple.audio",
    "com.apple.coreaudio",
    "com.apple.controlcenter",
    "com.apple.systemsound",
    "com.apple.VoiceMemos",
    "com.apple.QuickTimePlayerX",
    "dev.juanola.",
];

/// Apps conocidas: (prefijo del bundle id, nombre, tipo). El primer prefijo
/// que encaja gana; los procesos auxiliares (`….helper`, `….WebContent`)
/// comparten el prefijo de su app.
const KNOWN: &[(&str, &str, MeetingApp)] = &[
    ("us.zoom.", "Zoom", MeetingApp::Zoom),
    ("com.microsoft.teams", "Microsoft Teams", MeetingApp::Teams),
    ("com.tinyspeck.slackmacgap", "Slack", MeetingApp::SlackHuddle),
    ("com.apple.FaceTime", "FaceTime", MeetingApp::FaceTime),
    ("com.apple.TelephonyUtilities", "FaceTime", MeetingApp::FaceTime),
    ("net.whatsapp.WhatsApp", "WhatsApp", MeetingApp::WhatsApp),
    ("desktop.WhatsApp", "WhatsApp", MeetingApp::WhatsApp),
    ("WhatsApp", "WhatsApp", MeetingApp::WhatsApp),
    ("com.google.Chrome", "Google Chrome", MeetingApp::Browser),
    ("com.apple.Safari", "Safari", MeetingApp::Browser),
    // Safari reproduce y graba desde el proceso GPU de WebKit.
    ("com.apple.WebKit.GPU", "Safari", MeetingApp::Browser),
    ("company.thebrowser.Browser", "Arc", MeetingApp::Browser),
    ("com.microsoft.edgemac", "Microsoft Edge", MeetingApp::Browser),
    ("org.mozilla.firefox", "Firefox", MeetingApp::Browser),
    ("com.brave.Browser", "Brave", MeetingApp::Browser),
    ("com.vivaldi.Vivaldi", "Vivaldi", MeetingApp::Browser),
    ("com.operasoftware.Opera", "Opera", MeetingApp::Browser),
    ("app.zen-browser.zen", "Zen", MeetingApp::Browser),
    ("com.google.meet", "Google Meet", MeetingApp::GoogleMeet),
    ("com.cisco.webexmeetingsapp", "Webex", MeetingApp::Other),
    ("Cisco-Systems.Spark", "Webex", MeetingApp::Other),
    ("com.hnc.Discord", "Discord", MeetingApp::Other),
    ("com.skype.skype", "Skype", MeetingApp::Other),
    ("com.loom.desktop", "Loom", MeetingApp::Other),
    ("co.around.Around", "Around", MeetingApp::Other),
    ("com.ringcentral", "RingCentral", MeetingApp::Other),
    ("com.gotomeeting", "GoTo Meeting", MeetingApp::Other),
    ("ru.keepcoder.Telegram", "Telegram", MeetingApp::Other),
    ("org.telegram", "Telegram", MeetingApp::Other),
    ("org.whispersystems.signal-desktop", "Signal", MeetingApp::Other),
];

/// App detrás de un bundle id, o `None` si hay que ignorarla.
pub fn classify(bundle_id: &str) -> Option<DetectedApp> {
    if bundle_id.is_empty() || IGNORED_PREFIXES.iter().any(|p| bundle_id.starts_with(p)) {
        return None;
    }
    if let Some((prefix, name, kind)) = KNOWN.iter().find(|(p, ..)| bundle_id.starts_with(p)) {
        let base = prefix.trim_end_matches('.');
        return Some(DetectedApp { bundle_id: base.to_string(), name: name.to_string(), kind: kind.clone() });
    }
    // Desconocida: se avisa igual («Meeting detected»), con un nombre sacado
    // del bundle id. Los auxiliares (`com.foo.Bar.helper`) se agrupan con su app.
    let base = strip_helper(bundle_id);
    let name = base.rsplit('.').next().unwrap_or(base);
    Some(DetectedApp { bundle_id: base.to_string(), name: name.to_string(), kind: MeetingApp::Other })
}

fn strip_helper(bundle_id: &str) -> &str {
    let lower = bundle_id.to_ascii_lowercase();
    for marker in [".helper", ".webcontent", ".renderer", ".gpu", ".plugin"] {
        if let Some(i) = lower.find(marker) {
            return &bundle_id[..i];
        }
    }
    bundle_id
}

#[derive(Debug, Clone, Copy)]
pub struct WatcherConfig {
    /// Tiempo seguido con el micrófono antes de avisar (evita parpadeos, como
    /// el de una app que prueba el dispositivo al arrancar).
    pub min_on: Duration,
    /// Tiempo sin micrófono antes de dar la llamada por terminada (Zoom lo
    /// suelta y lo retoma al silenciarse o cambiar de dispositivo).
    pub grace: Duration,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self { min_on: Duration::from_secs(3), grace: Duration::from_secs(5) }
    }
}

#[derive(Debug)]
struct Track {
    app: DetectedApp,
    since: Duration,
    last_seen: Duration,
    announced: bool,
}

/// Máquina de estados sobre muestras periódicas de [`mic_users`]. No mira el
/// reloj: el llamante pasa el instante de cada muestra (así se prueba sin esperas).
#[derive(Debug, Default)]
pub struct MicWatcher {
    cfg: WatcherConfig,
    tracks: HashMap<String, Track>,
}

impl MicWatcher {
    pub fn new(cfg: WatcherConfig) -> Self {
        Self { cfg, tracks: HashMap::new() }
    }

    /// Apps que ya se anunciaron y siguen con el micrófono.
    pub fn active(&self) -> impl Iterator<Item = &DetectedApp> {
        self.tracks.values().filter(|t| t.announced).map(|t| &t.app)
    }

    /// Procesa una muestra tomada en `now` (desde cualquier origen fijo).
    pub fn update(&mut self, now: Duration, users: &[MicUser]) -> Vec<DetectEvent> {
        let mut out = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for u in users {
            let Some(app) = classify(&u.bundle_id) else { continue };
            if seen.contains(&app.bundle_id) {
                continue;
            }
            seen.push(app.bundle_id.clone());
            let t = self
                .tracks
                .entry(app.bundle_id.clone())
                .or_insert_with(|| Track { app: app.clone(), since: now, last_seen: now, announced: false });
            t.last_seen = now;
            if !t.announced && now.saturating_sub(t.since) >= self.cfg.min_on {
                t.announced = true;
                out.push(DetectEvent::MicInUse { app: t.app.clone() });
            }
        }
        let grace = self.cfg.grace;
        self.tracks.retain(|key, t| {
            if seen.contains(key) {
                return true;
            }
            if t.announced {
                if now.saturating_sub(t.last_seen) >= grace {
                    out.push(DetectEvent::MicReleased { app: t.app.clone() });
                    return false;
                }
                return true;
            }
            // Sin anunciar: un hueco corto reinicia la cuenta.
            false
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    fn user(bundle: &str) -> MicUser {
        MicUser { pid: 1, bundle_id: bundle.into() }
    }

    #[test]
    fn titles_match_granola() {
        assert_eq!(notification_title(&MeetingApp::SlackHuddle), "Huddle detected");
        assert_eq!(notification_title(&MeetingApp::FaceTime), "Call detected");
        assert_eq!(notification_title(&MeetingApp::Zoom), "Meeting detected");
    }

    #[test]
    fn classify_known_and_helpers() {
        let zoom = classify("us.zoom.xos").unwrap();
        assert_eq!((zoom.name.as_str(), zoom.kind), ("Zoom", MeetingApp::Zoom));
        let chrome = classify("com.google.Chrome.helper").unwrap();
        assert_eq!(chrome.bundle_id, "com.google.Chrome");
        assert_eq!(chrome.kind, MeetingApp::Browser);
        assert_eq!(classify("com.microsoft.teams2").unwrap().kind, MeetingApp::Teams);
        assert_eq!(classify("com.apple.WebKit.GPU").unwrap().name, "Safari");
        let other = classify("com.acme.Meetly.helper.Renderer").unwrap();
        assert_eq!((other.bundle_id.as_str(), other.name.as_str()), ("com.acme.Meetly", "Meetly"));
    }

    #[test]
    fn classify_ignores_system_listeners() {
        assert!(classify("com.apple.corespeechd").is_none());
        assert!(classify("com.apple.siri.embeddedspeech").is_none());
        assert!(classify("dev.juanola.desktop").is_none());
        assert!(classify("").is_none());
    }

    #[test]
    fn announces_after_min_on_and_releases_after_grace() {
        let mut w = MicWatcher::new(WatcherConfig::default());
        let zoom = [user("us.zoom.xos")];
        assert!(w.update(s(0), &zoom).is_empty());
        assert!(w.update(s(2), &zoom).is_empty());
        let ev = w.update(s(3), &zoom);
        assert!(matches!(&ev[..], [DetectEvent::MicInUse { app }] if app.name == "Zoom"));
        assert!(w.update(s(4), &zoom).is_empty(), "solo se anuncia una vez");
        // Hueco de 4 s (menos que grace): sigue activa.
        assert!(w.update(s(8), &[]).is_empty());
        assert!(w.update(s(9), &zoom).is_empty());
        assert_eq!(w.active().count(), 1);
        assert!(w.update(s(12), &[]).is_empty());
        let ev = w.update(s(14), &[]);
        assert!(matches!(&ev[..], [DetectEvent::MicReleased { .. }]));
        assert_eq!(w.active().count(), 0);
    }

    #[test]
    fn short_blips_never_announce() {
        let mut w = MicWatcher::new(WatcherConfig::default());
        let chrome = [user("com.google.Chrome.helper")];
        for t in 0..10 {
            let users: &[MicUser] = if t % 2 == 0 { &chrome } else { &[] };
            assert!(w.update(s(t), users).is_empty(), "t={t}");
        }
    }

    #[test]
    fn helpers_of_same_app_count_once() {
        let mut w = MicWatcher::new(WatcherConfig::default());
        let users = [user("com.google.Chrome.helper"), user("com.google.Chrome.helper.Renderer")];
        w.update(s(0), &users);
        let ev = w.update(s(3), &users);
        assert_eq!(ev.len(), 1);
    }
}
