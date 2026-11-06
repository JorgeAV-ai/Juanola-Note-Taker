//! Detección de reuniones: apps de videollamada en uso y micrófono activo por
//! otra app. Produce los títulos exactos de Granola para las notificaciones.

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum MeetingApp {
    Zoom,
    Teams,
    GoogleMeet,
    SlackHuddle,
    FaceTime,
    WhatsApp,
    Other(String),
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum DetectEvent {
    MeetingStarted { app: MeetingApp, title: Option<String> },
    MeetingEnded { app: MeetingApp },
    MicInUse { app_name: String },
    MicReleased,
}

/// Título de la notificación ad hoc, igual que en Granola.
pub fn notification_title(app: &MeetingApp) -> &'static str {
    match app {
        MeetingApp::SlackHuddle => "Huddle detected",
        MeetingApp::FaceTime | MeetingApp::WhatsApp => "Call detected",
        _ => "Meeting detected",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn titles_match_granola() {
        assert_eq!(notification_title(&MeetingApp::SlackHuddle), "Huddle detected");
        assert_eq!(notification_title(&MeetingApp::FaceTime), "Call detected");
        assert_eq!(notification_title(&MeetingApp::Zoom), "Meeting detected");
    }
}
