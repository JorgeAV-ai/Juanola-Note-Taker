//! Normalización inversa de texto (ITN) para números en español:
//! «mil ochocientas cuarenta» → «1840», «dos coma cuatro» → «2,4».
//!
//! El STT escribe muchas cifras con letras y LFM3-1.7B se equivoca al pasarlas
//! a dígitos al resumir (39 → 33). Convertirlas antes de dárselas al modelo
//! elimina ese error. Es conservador: «un», «uno» y «una» sueltos no se
//! convierten (son casi siempre artículos), solo dentro de un número mayor.

/// Valor de una palabra numérica.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Word {
    /// 0-99 sin multiplicador.
    Small(u64),
    /// 100-900.
    Hundred(u64),
    Thousand,
    Million,
    /// «y» entre decenas y unidades.
    And,
    /// «un»/«uno»/«una»: solo cuenta dentro de un número.
    One,
}

fn strip_accents(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' => 'u',
            'ü' => 'u',
            c => c,
        })
        .collect()
}

fn classify(raw: &str) -> Option<Word> {
    let w = strip_accents(&raw.to_lowercase());
    let small = match w.as_str() {
        "cero" => 0,
        "dos" => 2,
        "tres" => 3,
        "cuatro" => 4,
        "cinco" => 5,
        "seis" => 6,
        "siete" => 7,
        "ocho" => 8,
        "nueve" => 9,
        "diez" => 10,
        "once" => 11,
        "doce" => 12,
        "trece" => 13,
        "catorce" => 14,
        "quince" => 15,
        "dieciseis" => 16,
        "diecisiete" => 17,
        "dieciocho" => 18,
        "diecinueve" => 19,
        "veinte" => 20,
        "veintiun" | "veintiuno" | "veintiuna" => 21,
        "veintidos" => 22,
        "veintitres" => 23,
        "veinticuatro" => 24,
        "veinticinco" => 25,
        "veintiseis" => 26,
        "veintisiete" => 27,
        "veintiocho" => 28,
        "veintinueve" => 29,
        "treinta" => 30,
        "cuarenta" => 40,
        "cincuenta" => 50,
        "sesenta" => 60,
        "setenta" => 70,
        "ochenta" => 80,
        "noventa" => 90,
        "cien" | "ciento" => return Some(Word::Hundred(100)),
        "doscientos" | "doscientas" => return Some(Word::Hundred(200)),
        "trescientos" | "trescientas" => return Some(Word::Hundred(300)),
        "cuatrocientos" | "cuatrocientas" => return Some(Word::Hundred(400)),
        "quinientos" | "quinientas" => return Some(Word::Hundred(500)),
        "seiscientos" | "seiscientas" => return Some(Word::Hundred(600)),
        "setecientos" | "setecientas" => return Some(Word::Hundred(700)),
        "ochocientos" | "ochocientas" => return Some(Word::Hundred(800)),
        "novecientos" | "novecientas" => return Some(Word::Hundred(900)),
        "mil" => return Some(Word::Thousand),
        "millon" | "millones" => return Some(Word::Million),
        "y" => return Some(Word::And),
        "un" | "uno" | "una" => return Some(Word::One),
        _ => return None,
    };
    Some(Word::Small(small))
}

/// Interpreta una secuencia de palabras numéricas ya validada.
fn value(words: &[Word]) -> u64 {
    let (mut total, mut current) = (0u64, 0u64);
    for w in words {
        match *w {
            Word::Small(v) | Word::Hundred(v) => current += v,
            Word::One => current += 1,
            Word::And => {}
            Word::Thousand => {
                total += current.max(1) * 1000;
                current = 0;
            }
            Word::Million => {
                total = (total + current.max(1)) * 1_000_000;
                current = 0;
            }
        }
    }
    total + current
}

/// Longitud del número más largo que empieza en `words[0]` (0 si no hay).
/// Gramática simplificada del español: [centenas] [decenas [y unidades] | 0-29]
/// [mil [...]] , con «un»/«una» solo delante de «mil»/«millón» o tras «y».
fn number_len(words: &[Option<Word>]) -> usize {
    let mut i = 0;
    let mut last_ok = 0;
    let mut has_strong = false;
    // Hasta dos grupos: «... millones ... mil ...» basta para reuniones.
    for _group in 0..3 {
        let start = i;
        if let Some(Some(Word::Hundred(_))) = words.get(i) {
            i += 1;
            has_strong = true;
        }
        match words.get(i) {
            Some(Some(Word::Small(v))) => {
                let tens = *v >= 30 && v % 10 == 0;
                i += 1;
                has_strong = true;
                if tens {
                    if let (Some(Some(Word::And)), Some(Some(Word::Small(u)))) = (words.get(i), words.get(i + 1)) {
                        if (1..=9).contains(u) {
                            i += 2;
                        }
                    } else if let (Some(Some(Word::And)), Some(Some(Word::One))) = (words.get(i), words.get(i + 1)) {
                        i += 2;
                    }
                }
            }
            Some(Some(Word::One)) if matches!(words.get(i + 1), Some(Some(Word::Thousand | Word::Million))) => {
                i += 1;
            }
            _ => {}
        }
        if i > start && has_strong {
            last_ok = i;
        }
        match words.get(i) {
            Some(Some(Word::Thousand | Word::Million)) => {
                i += 1;
                has_strong = true;
                last_ok = i;
            }
            _ => break,
        }
    }
    last_ok
}

/// Números escritos con letras a dígitos según el idioma del resumen: en
/// castellano el ITN de este módulo; en inglés `text-processing-rs` (el ITN de
/// NeMo portado a Rust). En los demás idiomas, el texto tal cual.
pub fn normalize_numbers_lang(text: &str, language: &str) -> String {
    match crate::language::language_index(language) {
        0 => normalize_numbers(text),
        1 => text_processing_rs::normalize_sentence_lang(text, "en"),
        _ => text.to_owned(),
    }
}

/// Sustituye los números escritos con letras por dígitos. Mantiene el resto
/// del texto (puntuación, mayúsculas) intacto.
pub fn normalize_numbers(text: &str) -> String {
    // Trocear en palabras y separadores conservando todo.
    let mut tokens: Vec<(bool, &str)> = Vec::new();
    let mut start = 0;
    let mut in_word = false;
    for (i, c) in text.char_indices() {
        let w = c.is_alphabetic();
        if i == 0 {
            in_word = w;
        } else if w != in_word {
            tokens.push((in_word, &text[start..i]));
            start = i;
            in_word = w;
        }
    }
    if !text.is_empty() {
        tokens.push((in_word, &text[start..]));
    }

    // Índices de los tokens palabra y su clasificación.
    let word_idx: Vec<usize> = tokens.iter().enumerate().filter(|(_, t)| t.0).map(|(i, _)| i).collect();
    let classes: Vec<Option<Word>> = word_idx.iter().map(|&i| classify(tokens[i].1)).collect();
    // Un número solo continúa si entre palabras hay únicamente espacios.
    let joined_by_space = |a: usize, b: usize| tokens[a + 1..b].iter().all(|t| !t.0 && t.1.trim().is_empty());

    let mut out = String::with_capacity(text.len());
    let mut emitted = 0; // siguiente token por emitir
    let mut w = 0;
    while w < word_idx.len() {
        // Cortar la ventana en el primer separador que no sea espacio.
        let mut end = w + 1;
        while end < word_idx.len() && joined_by_space(word_idx[end - 1], word_idx[end]) {
            end += 1;
        }
        let mut n = number_len(&classes[w..end]);
        // «ciento» suelto («por ciento») no es un número: exige algo detrás.
        if n == 1 && strip_accents(&tokens[word_idx[w]].1.to_lowercase()) == "ciento" {
            n = 0;
        }
        if n == 0 {
            w += 1;
            continue;
        }
        let words: Vec<Word> = classes[w..w + n].iter().map(|c| c.unwrap()).collect();
        // «112 mil», «2,5 millones»: dígitos delante de un multiplicador suelto.
        if n == 1 && matches!(words[0], Word::Thousand | Word::Million) {
            let mut pending = String::new();
            for t in &tokens[emitted..word_idx[w]] {
                pending.push_str(t.1);
            }
            let body = pending.strip_suffix(' ').unwrap_or("");
            // Tras el último carácter que no es cifra (que puede ocupar varios
            // bytes: «≈112 mil», «€112 mil»).
            let num_start =
                body.char_indices().rev().find(|(_, c)| !(c.is_ascii_digit() || *c == ',')).map_or(0, |(i, c)| i + c.len_utf8());
            let digits = &body[num_start..];
            if let Ok(v) = digits.replace(',', ".").parse::<f64>() {
                if !digits.is_empty() && !digits.starts_with(',') {
                    let mult = if words[0] == Word::Thousand { 1e3 } else { 1e6 };
                    out.push_str(&body[..num_start]);
                    out.push_str(&format!("{}", (v * mult).round() as u64));
                    emitted = word_idx[w] + 1;
                    w += 1;
                    continue;
                }
            }
        }
        let mut repl = value(&words).to_string();
        let mut last = w + n - 1;
        // Decimales: «dos coma cuatro» → «2,4».
        if w + n + 1 < word_idx.len()
            && joined_by_space(word_idx[last], word_idx[w + n])
            && strip_accents(&tokens[word_idx[w + n]].1.to_lowercase()) == "coma"
            && joined_by_space(word_idx[w + n], word_idx[w + n + 1])
        {
            let mut e2 = w + n + 2;
            while e2 < word_idx.len() && joined_by_space(word_idx[e2 - 1], word_idx[e2]) {
                e2 += 1;
            }
            // Tras «coma», «uno» sí es un número («tres coma uno»).
            let m = match number_len(&classes[w + n + 1..e2]) {
                0 if classes[w + n + 1] == Some(Word::One) => 1,
                m => m,
            };
            if m > 0 {
                let dec: Vec<Word> = classes[w + n + 1..w + n + 1 + m].iter().map(|c| c.unwrap()).collect();
                repl = format!("{repl},{}", value(&dec));
                last = w + n + m;
            }
        }
        for t in &tokens[emitted..word_idx[w]] {
            out.push_str(t.1);
        }
        out.push_str(&repl);
        emitted = word_idx[last] + 1;
        w = last + 1;
    }
    for t in &tokens[emitted..] {
        out.push_str(t.1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::normalize_numbers as n;

    #[test]
    fn basic_numbers() {
        assert_eq!(n("un plan básico de treinta y nueve euros"), "un plan básico de 39 euros");
        assert_eq!(n("setenta y nueve euros hasta cinco fisioterapeutas"), "79 euros hasta 5 fisioterapeutas");
        assert_eq!(n("ciento cuarenta y nueve euros"), "149 euros");
        assert_eq!(n("Cerramos con mil ochocientas cuarenta clínicas"), "Cerramos con 1840 clínicas");
        assert_eq!(n("llegaron a ciento doce mil euros"), "llegaron a 112000 euros");
        assert_eq!(n("una oferta de cincuenta y dos mil euros"), "una oferta de 52000 euros");
        assert_eq!(n("unos mil cuatrocientos euros al mes"), "unos 1400 euros al mes");
        assert_eq!(n("el dieciocho de noviembre"), "el 18 de noviembre");
        assert_eq!(n("el veintidós por ciento"), "el 22 por ciento");
        assert_eq!(n("un millón de usuarios"), "1000000 de usuarios");
    }

    #[test]
    fn decimals_and_punctuation() {
        assert_eq!(n("del tres coma uno al dos coma cuatro por ciento."), "del 3,1 al 2,4 por ciento.");
        assert_eq!(n("Doce, trece y catorce"), "12, 13 y 14");
    }

    #[test]
    fn articles_are_kept() {
        assert_eq!(n("un único plan y una persona más"), "un único plan y una persona más");
        assert_eq!(n("Uno de los problemas"), "Uno de los problemas");
        assert_eq!(n("tenemos una candidata"), "tenemos una candidata");
        assert_eq!(n("treinta y una clínicas"), "31 clínicas");
    }

    #[test]
    fn digits_before_multiplier() {
        assert_eq!(n("llegaron a 112 mil euros"), "llegaron a 112000 euros");
        assert_eq!(n("unos 2,5 millones y 35 mil más"), "unos 2500000 y 35000 más");
        assert_eq!(n("Tenemos aprobado los 35 mil euros"), "Tenemos aprobado los 35000 euros");
        // Un carácter de varios bytes justo antes de la cifra no la parte.
        assert_eq!(n("un coste de ≈180 mil al mes"), "un coste de ≈180000 al mes");
        assert_eq!(n("pagan €112 mil"), "pagan €112000");
        assert_eq!(n("≈ mil"), "≈ 1000");
    }

    #[test]
    fn digits_and_other_text_untouched() {
        assert_eq!(n("El 15 de enero, plan centro"), "El 15 de enero, plan centro");
        assert_eq!(n(""), "");
        assert_eq!(n("¿Cuánto cuesta?"), "¿Cuánto cuesta?");
    }
}
