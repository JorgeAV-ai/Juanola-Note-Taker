//! Contexto para el prompt de Qwen3-ASR: lo que se sabe de la reunión antes de
//! oírla (título, asistentes, descripción del evento, notas del usuario y su
//! glosario). Con los términos de la reunión, el 1.7B acierta casi todos los
//! nombres propios y baja el WER a casi la mitad; el título y los asistentes
//! solos apenas cambian nada (`docs/experimentos.md`, 9 nov).

/// Límites para que un evento con una descripción enorme no infle el prompt
/// de cada frase.
const MAX_DESCRIPTION: usize = 600;
const MAX_NOTES: usize = 1000;
const MAX_TERMS: usize = 200;

#[derive(Debug, Clone, Default)]
pub struct AsrContext {
    pub title: String,
    /// Nombres de los asistentes (sin correos).
    pub attendees: Vec<String>,
    /// Descripción del evento de calendario.
    pub description: String,
    /// Texto de las notas que el usuario ya escribió.
    pub notes: String,
    /// Términos del glosario del usuario: nombres, productos, siglas.
    pub glossary: Vec<String>,
}

impl AsrContext {
    /// Texto para el mensaje de sistema; `None` si no hay nada que decir.
    pub fn render(&self) -> Option<String> {
        let mut lines = Vec::new();
        let title = self.title.trim();
        if !title.is_empty() {
            lines.push(format!("Reunión: {title}"));
        }
        let mut seen = std::collections::HashSet::new();
        let attendees: Vec<&str> =
            self.attendees.iter().map(|a| a.trim()).filter(|a| !a.is_empty() && seen.insert(a.to_lowercase())).collect();
        if !attendees.is_empty() {
            lines.push(format!("Asistentes: {}", attendees.join(", ")));
        }
        let description = clean_description(&self.description);
        if !description.is_empty() {
            lines.push(format!("Descripción: {}", truncate(&description, MAX_DESCRIPTION)));
        }
        let notes = self.notes.split_whitespace().collect::<Vec<_>>().join(" ");
        if !notes.is_empty() {
            lines.push(format!("Notas: {}", truncate(&notes, MAX_NOTES)));
        }
        let mut seen = std::collections::HashSet::new();
        let terms: Vec<&str> =
            self.glossary.iter().map(|t| t.trim()).filter(|t| !t.is_empty() && seen.insert(t.to_lowercase())).take(MAX_TERMS).collect();
        if !terms.is_empty() {
            lines.push(format!("Términos: {}.", terms.join(", ")));
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }
}

/// Glosario escrito por el usuario: un término por línea o separados por comas.
pub fn parse_glossary(text: &str) -> Vec<String> {
    text.split(['\n', ',', ';']).map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned).collect()
}

/// Quita de la descripción las líneas con enlaces o con datos de acceso a la
/// videollamada (ID, código, teléfonos): no aportan palabras de la reunión.
fn clean_description(text: &str) -> String {
    const NOISE: [&str; 12] =
        ["http", "www.", "meeting id", "passcode", "contraseña", "código", "pin:", "dial", "join", "unirse", "zoom", "teams"];
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| {
            let low = l.to_lowercase();
            let digits = l.chars().filter(char::is_ascii_digit).count();
            !NOISE.iter().any(|n| low.contains(n)) && digits * 3 < l.chars().count()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let cut: String = text.chars().take(max).collect();
    // Cortar en un espacio para no dejar media palabra.
    match cut.rfind(' ') {
        Some(i) => format!("{}…", &cut[..i]),
        None => format!("{cut}…"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vacio_no_da_contexto() {
        assert_eq!(AsrContext::default().render(), None);
        let ctx = AsrContext { title: "  ".into(), glossary: vec![" ".into()], ..Default::default() };
        assert_eq!(ctx.render(), None);
    }

    #[test]
    fn formato_y_duplicados() {
        let ctx = AsrContext {
            title: "Demo de Rutario".into(),
            attendees: vec!["Patricia".into(), "Raúl".into(), "patricia".into()],
            glossary: parse_glossary("Rutario, SAP S/4HANA\nÍñigo Etxeberria\n\nrutario"),
            ..Default::default()
        };
        assert_eq!(
            ctx.render().unwrap(),
            "Reunión: Demo de Rutario\nAsistentes: Patricia, Raúl\nTérminos: Rutario, SAP S/4HANA, Íñigo Etxeberria."
        );
    }

    #[test]
    fn descripcion_sin_datos_de_la_videollamada() {
        let d = "Revisar la integración con SAP.\nJoin Zoom Meeting\nhttps://zoom.us/j/123\nMeeting ID: 123 456 7890\n+34 910 123 456";
        let ctx = AsrContext { description: d.into(), ..Default::default() };
        assert_eq!(ctx.render().unwrap(), "Descripción: Revisar la integración con SAP.");
    }

    #[test]
    fn notas_largas_se_cortan() {
        let ctx = AsrContext { notes: "palabra ".repeat(500), ..Default::default() };
        let out = ctx.render().unwrap();
        assert!(out.chars().count() < MAX_NOTES + 20);
        assert!(out.ends_with("palabra…"));
    }
}
