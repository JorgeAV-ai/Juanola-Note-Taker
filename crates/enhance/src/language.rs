//! Idioma del resumen a partir de la transcripción.
//!
//! Los segmentos no guardan el idioma detectado, y el usuario puede grabar con
//! «Detectar». Basta con contar palabras funcionales frecuentes de cada idioma:
//! en una reunión hay cientos y el recuento no deja dudas.

use juanola_core_types::Segment;

/// (nombre para el prompt, que está en español; palabras funcionales).
const LANGUAGES: &[(&str, &[&str])] = &[
    ("español", &["el", "la", "los", "las", "que", "de", "y", "en", "un", "una", "es", "por", "para", "con", "no", "lo", "se", "del", "pero", "muy", "está", "hay", "también"]),
    ("inglés", &["the", "and", "is", "are", "to", "of", "that", "it", "we", "you", "this", "with", "for", "have", "be", "not", "was", "what", "will", "they"]),
    ("portugués", &["o", "os", "as", "que", "de", "e", "em", "um", "uma", "é", "não", "do", "da", "para", "com", "isso", "também", "muito", "você", "está"]),
    ("francés", &["le", "la", "les", "et", "est", "des", "une", "un", "que", "pas", "pour", "dans", "nous", "vous", "ce", "avec", "sur", "mais", "très", "il"]),
    ("alemán", &["der", "die", "das", "und", "ist", "nicht", "ich", "wir", "sie", "ein", "eine", "zu", "mit", "auf", "den", "auch", "es", "für", "von", "haben"]),
    ("italiano", &["il", "la", "che", "di", "e", "è", "un", "una", "per", "non", "sono", "con", "del", "della", "anche", "questo", "abbiamo", "gli", "molto", "noi"]),
];

/// Idioma en que escribir el resumen; español si la transcripción no da pistas.
pub fn summary_language(segments: &[Segment]) -> &'static str {
    let mut counts = [0usize; LANGUAGES.len()];
    for seg in segments {
        for word in seg.text.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()) {
            let w = word.to_lowercase();
            for (i, (_, words)) in LANGUAGES.iter().enumerate() {
                if words.contains(&w.as_str()) {
                    counts[i] += 1;
                }
            }
        }
    }
    let (best, n) = counts.iter().enumerate().max_by_key(|(_, n)| **n).expect("hay idiomas");
    if *n < 5 {
        return LANGUAGES[0].0;
    }
    LANGUAGES[best].0
}

/// Texto que se añade a los prompts (escritos en español) cuando el resumen va
/// en otro idioma: sin él, el modelo mezcla las etiquetas «Yo»/«Ellos» en las
/// viñetas. El título «Próximos pasos» se mantiene porque marca las tareas; se
/// traduce al final con [`localize_heading`].
pub fn prompt_suffix(language: &str) -> String {
    if language == LANGUAGES[0].0 {
        return String::new();
    }
    format!(
        "\nEscribe todas las viñetas y los títulos en {language} (salvo `## Próximos pasos`, que va tal cual). \
         No escribas «Yo» ni «Ellos» en las notas: nombra a las personas o usa el equivalente en {language} de «yo» y «el equipo»."
    )
}

/// Títulos fijos del pipeline («Próximos pasos», «Tus notas») en el idioma del resumen.
pub fn localize_heading(heading: &str, language: &str) -> String {
    let t: Option<(&str, &str)> = match language {
        "inglés" => Some(("Next steps", "Your notes")),
        "portugués" => Some(("Próximos passos", "Suas notas")),
        "francés" => Some(("Prochaines étapes", "Vos notes")),
        "alemán" => Some(("Nächste Schritte", "Deine Notizen")),
        "italiano" => Some(("Prossimi passi", "Le tue note")),
        _ => None,
    };
    match (heading, t) {
        ("Próximos pasos", Some((steps, _))) => steps.into(),
        ("Tus notas", Some((_, notes))) => notes.into(),
        _ => heading.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use juanola_core_types::{Channel, Id};

    fn segs(texts: &[&str]) -> Vec<Segment> {
        texts
            .iter()
            .map(|t| Segment {
                id: Id::new_v4(),
                note_id: Id::nil(),
                channel: Channel::System,
                speaker_id: None,
                start: 0.0,
                end: 1.0,
                text: (*t).into(),
                confidence: None,
                is_final: true,
            })
            .collect()
    }

    #[test]
    fn detects_languages() {
        let es = segs(&["Las ventas de octubre han subido y el equipo está contento con los resultados para el trimestre."]);
        assert_eq!(summary_language(&es), "español");
        let en = segs(&["The sales are up and we have to prepare the deck for Friday, that is what we agreed with the team."]);
        assert_eq!(summary_language(&en), "inglés");
        let pt = segs(&["Nós vamos lançar a versão nova em novembro e o time está muito contente, isso é ótimo para você."]);
        assert_eq!(summary_language(&pt), "portugués");
        let de = segs(&["Wir haben die Zahlen von Oktober und das ist nicht schlecht, die Präsentation ist für Freitag und wir sind auch bereit."]);
        assert_eq!(summary_language(&de), "alemán");
    }

    #[test]
    fn spanish_prompt_unchanged() {
        assert_eq!(prompt_suffix("español"), "");
        assert!(prompt_suffix("inglés").contains("inglés"));
        assert_eq!(localize_heading("Próximos pasos", "inglés"), "Next steps");
        assert_eq!(localize_heading("Próximos pasos", "español"), "Próximos pasos");
        assert_eq!(localize_heading("Ventas", "inglés"), "Ventas");
    }

    #[test]
    fn defaults_to_spanish() {
        assert_eq!(summary_language(&segs(&["OK", "vale"])), "español");
        assert_eq!(summary_language(&[]), "español");
    }
}
