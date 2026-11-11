//! Idioma del resumen a partir de la transcripción.
//!
//! Los segmentos no guardan el idioma detectado, y el usuario puede grabar con
//! «Detectar». Basta con contar palabras funcionales frecuentes de cada idioma:
//! en una reunión hay cientos y el recuento no deja dudas.

use juanola_core_types::Segment;

/// (nombre para el prompt, que está en español; palabras funcionales).
const LANGUAGES: &[(&str, &[&str])] = &[
    (
        "español",
        &[
            "el", "la", "los", "las", "que", "de", "y", "en", "un", "una", "es", "por", "para", "con", "no", "lo", "se", "del", "pero",
            "muy", "está", "hay", "también",
        ],
    ),
    (
        "inglés",
        &[
            "the", "and", "is", "are", "to", "of", "that", "it", "we", "you", "this", "with", "for", "have", "be", "not", "was", "what",
            "will", "they",
        ],
    ),
    (
        "portugués",
        &[
            "o", "os", "as", "que", "de", "e", "em", "um", "uma", "é", "não", "do", "da", "para", "com", "isso", "também", "muito", "você",
            "está",
        ],
    ),
    (
        "francés",
        &[
            "le", "la", "les", "et", "est", "des", "une", "un", "que", "pas", "pour", "dans", "nous", "vous", "ce", "avec", "sur", "mais",
            "très", "il",
        ],
    ),
    (
        "alemán",
        &[
            "der", "die", "das", "und", "ist", "nicht", "ich", "wir", "sie", "ein", "eine", "zu", "mit", "auf", "den", "auch", "es", "für",
            "von", "haben",
        ],
    ),
    (
        "italiano",
        &[
            "il", "la", "che", "di", "e", "è", "un", "una", "per", "non", "sono", "con", "del", "della", "anche", "questo", "abbiamo",
            "gli", "molto", "noi",
        ],
    ),
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
    // `max_by_key` gana el último en empate; preferir el primero (español).
    let (best, n) = counts.iter().enumerate().max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(&a.0))).expect("hay idiomas");
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

/// Variante de [`prompt_suffix`] con plantilla: los títulos ya vienen en el
/// idioma del resumen (o como los escribió el usuario) y no se traducen.
pub fn template_prompt_suffix(language: &str) -> String {
    if language == LANGUAGES[0].0 {
        return String::new();
    }
    format!(
        "\nEscribe todas las viñetas en {language}. Usa los títulos de los apartados exactamente como están, sin traducirlos ni cambiarlos. \
         No escribas «Yo» ni «Ellos» en las notas: nombra a las personas o usa el equivalente en {language} de «yo» y «el equipo»."
    )
}

/// Nombres de los idiomas tal como los devuelve [`summary_language`], en el
/// orden en que las plantillas integradas guardan sus títulos.
pub const LANGUAGE_NAMES: [&str; 6] = ["español", "inglés", "portugués", "francés", "alemán", "italiano"];

/// Posición de `language` en [`LANGUAGE_NAMES`]; 0 (español) si no se reconoce.
pub fn language_index(language: &str) -> usize {
    LANGUAGE_NAMES.iter().position(|l| *l == language).unwrap_or(0)
}

/// Títulos fijos del pipeline: «Próximos pasos», «Tus notas» y «Otros temas».
const FIXED_HEADINGS: [[&str; 6]; 3] = [
    ["Próximos pasos", "Next steps", "Próximos passos", "Prochaines étapes", "Nächste Schritte", "Prossimi passi"],
    ["Tus notas", "Your notes", "Suas notas", "Vos notes", "Deine Notizen", "Le tue note"],
    ["Otros temas", "Other topics", "Outros temas", "Autres sujets", "Weitere Themen", "Altri argomenti"],
];

/// Viñeta de un apartado de la plantilla del que no se habló.
const NOT_DISCUSSED: [&str; 6] =
    ["No se habló de esto", "Not discussed", "Não se falou disto", "Pas abordé", "Nicht besprochen", "Non se n'è parlato"];

/// Títulos fijos del pipeline («Próximos pasos», «Tus notas», «Otros temas»)
/// en el idioma del resumen. Los demás títulos se devuelven tal cual.
pub fn localize_heading(heading: &str, language: &str) -> String {
    let lang = language_index(language);
    match FIXED_HEADINGS.iter().find(|h| h[0] == heading) {
        Some(h) => h[lang].into(),
        None => heading.into(),
    }
}

/// ¿Es «Otros temas» en alguno de los idiomas?
pub fn is_other_topics(heading: &str) -> bool {
    let h = juanola_core_types::fold(heading);
    FIXED_HEADINGS[2].iter().any(|o| juanola_core_types::fold(o) == h)
}

/// «No se habló de esto» en el idioma del resumen.
pub fn not_discussed(language: &str) -> &'static str {
    NOT_DISCUSSED[language_index(language)]
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
        let de = segs(&[
            "Wir haben die Zahlen von Oktober und das ist nicht schlecht, die Präsentation ist für Freitag und wir sind auch bereit.",
        ]);
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
    fn names_follow_detection_order() {
        let detected: Vec<&str> = LANGUAGES.iter().map(|(n, _)| *n).collect();
        assert_eq!(detected, LANGUAGE_NAMES);
        assert_eq!(localize_heading("Otros temas", "alemán"), "Weitere Themen");
        assert_eq!(not_discussed("inglés"), "Not discussed");
        assert_eq!(not_discussed("klingon"), "No se habló de esto");
    }

    #[test]
    fn defaults_to_spanish() {
        assert_eq!(summary_language(&segs(&["OK", "vale"])), "español");
        assert_eq!(summary_language(&[]), "español");
    }
}
