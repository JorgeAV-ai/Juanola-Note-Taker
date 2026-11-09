//! Comparación de transcripciones del mismo audio: frente a una referencia
//! (WER y CER) o entre dos modelos (dónde no coinciden y cómo combinarlos).
//!
//! Se compara texto normalizado (minúsculas, sin puntuación ni tildes) para
//! no contar diferencias de formato, y las cifras dichas con letras se pasan a
//! dígitos (solo español por ahora), para que «85» y «ochenta y cinco» coincidan.

use std::ops::Range;

/// Palabra con su forma original (para mostrarla) y la normalizada (para comparar).
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub key: String,
}

/// Parte un texto en palabras. Los signos de puntuación se quedan pegados al
/// original pero no a la clave; las palabras que solo son puntuación se
/// descartan. Las cifras dichas con letras se unen en una sola palabra con
/// clave en dígitos («treinta y cinco mil» → `35000`), y «%» cuenta como
/// «por ciento», para que «35.700» y «treinta y cinco mil setecientos» coincidan.
pub fn words(text: &str) -> Vec<Word> {
    words_with(text, true)
}

/// Como [`words`]; con `numbers = false` no se unen las cifras ni se expande
/// «%» (sirve para medir cuánto aporta el normalizador).
pub fn words_with(text: &str, numbers: bool) -> Vec<Word> {
    let mut raw = Vec::new();
    for w in text.split_whitespace() {
        let key = normalize(w);
        if key.is_empty() {
            continue;
        }
        raw.push(Word { text: w.to_owned(), key });
        if numbers && w.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '%').ends_with('%') {
            for k in ["por", "ciento"] {
                raw.push(Word { text: String::new(), key: k.into() });
            }
        }
    }
    if numbers { join_numbers(raw) } else { raw }
}

/// Valor de una palabra numérica en español; `None` si no lo es.
fn number_word(k: &str) -> Option<u64> {
    if !k.is_empty() && k.chars().all(|c| c.is_ascii_digit()) {
        return k.parse().ok();
    }
    const UNITS: [&str; 30] = [
        "cero", "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve", "diez", "once", "doce",
        "trece", "catorce", "quince", "dieciseis", "diecisiete", "dieciocho", "diecinueve", "veinte", "veintiuno",
        "veintidos", "veintitres", "veinticuatro", "veinticinco", "veintiseis", "veintisiete", "veintiocho",
        "veintinueve",
    ];
    const TENS: [&str; 7] = ["treinta", "cuarenta", "cincuenta", "sesenta", "setenta", "ochenta", "noventa"];
    const HUNDREDS: [&str; 9] = [
        "ciento", "doscientos", "trescientos", "cuatrocientos", "quinientos", "seiscientos", "setecientos",
        "ochocientos", "novecientos",
    ];
    let fem = |s: &str| s.replace("ientas", "ientos");
    if let Some(i) = UNITS.iter().position(|u| *u == k) {
        return Some(i as u64);
    }
    if let Some(i) = TENS.iter().position(|u| *u == k) {
        return Some(30 + 10 * i as u64);
    }
    if let Some(i) = HUNDREDS.iter().position(|u| *u == fem(k)) {
        return Some(100 * (i as u64 + 1));
    }
    match k {
        "cien" => Some(100),
        "una" | "veintiun" | "veintiuna" => Some(if k == "una" { 1 } else { 21 }),
        _ => None,
    }
}

/// Une las secuencias de palabras numéricas en una sola palabra.
fn join_numbers(ws: Vec<Word>) -> Vec<Word> {
    let is_mult = |k: &str| matches!(k, "mil" | "millon" | "millones");
    let mut out = Vec::with_capacity(ws.len());
    let mut i = 0;
    while i < ws.len() {
        // «un» y «una» solo cuentan como cifra delante de «mil» o «millón»: si
        // no, «un almacén» se volvería «1 almacén».
        let starts = number_word(&ws[i].key).is_some_and(|_| ws[i].key != "una")
            || (matches!(ws[i].key.as_str(), "un" | "una") && ws.get(i + 1).is_some_and(|w| is_mult(&w.key)))
            || ws[i].key == "mil";
        if !starts {
            out.push(ws[i].clone());
            i += 1;
            continue;
        }
        let (mut total, mut cur, mut j) = (0u64, 0u64, i);
        let mut last_tens = false;
        while j < ws.len() {
            let k = ws[j].key.as_str();
            if let Some(v) = number_word(k).or(if k == "un" { Some(1) } else { None }) {
                // Dos cifras seguidas sin conector («dos tres») son dos números,
                // salvo que la primera sea una centena o decena compatible.
                if j > i && cur != 0 && !(cur % 100 == 0 && v < 100 || last_tens && v < 10) {
                    break;
                }
                cur += v;
                last_tens = (30..=90).contains(&v) && v % 10 == 0;
            } else if k == "y" && last_tens && ws.get(j + 1).and_then(|w| number_word(&w.key)).is_some_and(|v| (1..10).contains(&v)) {
                last_tens = true; // la «y» de «treinta y cinco»
            } else if k == "mil" {
                total += cur.max(1) * 1000;
                cur = 0;
                last_tens = false;
            } else if matches!(k, "millon" | "millones") {
                total = (total + cur.max(1)) * 1_000_000;
                cur = 0;
                last_tens = false;
            } else {
                break;
            }
            j += 1;
        }
        let text = ws[i..j].iter().map(|w| w.text.as_str()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
        out.push(Word { text, key: (total + cur).to_string() });
        i = j;
    }
    out
}

/// Minúsculas, sin tildes ni puntuación. Se conservan letras y dígitos (y la
/// ñ, que en español distingue palabras: «año» frente a «ano»).
pub fn normalize(w: &str) -> String {
    w.chars()
        .flat_map(char::to_lowercase)
        .filter_map(|c| match c {
            'á' | 'à' | 'â' | 'ä' => Some('a'),
            'é' | 'è' | 'ê' | 'ë' => Some('e'),
            'í' | 'ì' | 'î' | 'ï' => Some('i'),
            'ó' | 'ò' | 'ô' | 'ö' => Some('o'),
            'ú' | 'ù' | 'û' | 'ü' => Some('u'),
            'ç' => Some('c'),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Same,
    /// Palabra distinta en la misma posición.
    Sub,
    /// Palabra solo en `a`.
    OnlyA,
    /// Palabra solo en `b`.
    OnlyB,
}

/// Alineación de mínima distancia de edición entre dos secuencias de claves.
/// Devuelve las operaciones en orden; `Same` y `Sub` avanzan en las dos.
pub fn align(a: &[Word], b: &[Word]) -> Vec<Op> {
    let (n, m) = (a.len(), b.len());
    // d[i][j] = coste de alinear a[..i] con b[..j].
    let mut d = vec![vec![0u32; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i as u32;
    }
    for j in 0..=m {
        d[0][j] = j as u32;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub = d[i - 1][j - 1] + u32::from(a[i - 1].key != b[j - 1].key);
            d[i][j] = sub.min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
        }
    }
    let mut ops = Vec::with_capacity(n.max(m));
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && d[i][j] == d[i - 1][j - 1] + u32::from(a[i - 1].key != b[j - 1].key) {
            ops.push(if a[i - 1].key == b[j - 1].key { Op::Same } else { Op::Sub });
            i -= 1;
            j -= 1;
        } else if i > 0 && d[i][j] == d[i - 1][j] + 1 {
            ops.push(Op::OnlyA);
            i -= 1;
        } else {
            ops.push(Op::OnlyB);
            j -= 1;
        }
    }
    ops.reverse();
    ops
}

/// Tramo en el que las dos transcripciones no coinciden: índices de palabra en
/// cada una. Un rango vacío es un hueco (el otro modelo oyó algo que este no).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    pub a: Range<usize>,
    pub b: Range<usize>,
}

/// Agrupa las operaciones distintas de `Same` consecutivas en tramos.
pub fn diffs(ops: &[Op]) -> Vec<Diff> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    let mut open: Option<(usize, usize)> = None;
    for op in ops {
        if *op == Op::Same {
            if let Some((si, sj)) = open.take() {
                out.push(Diff { a: si..i, b: sj..j });
            }
        } else if open.is_none() {
            open = Some((i, j));
        }
        match op {
            Op::Same | Op::Sub => {
                i += 1;
                j += 1;
            }
            Op::OnlyA => i += 1,
            Op::OnlyB => j += 1,
        }
    }
    if let Some((si, sj)) = open {
        out.push(Diff { a: si..i, b: sj..j });
    }
    out
}

/// Errores de palabra de `hyp` frente a `reference`: sustituciones +
/// inserciones + borrados (el numerador del WER).
pub fn edit_errors(reference: &[Word], hyp: &[Word]) -> usize {
    align(reference, hyp).iter().filter(|o| **o != Op::Same).count()
}

/// Errores de carácter de `hyp` frente a `reference` (numerador del CER) y
/// número de caracteres de la referencia. Se comparan las claves
/// normalizadas unidas por un espacio, así que el CER tampoco cuenta tildes,
/// mayúsculas ni puntuación.
pub fn char_errors(reference: &[Word], hyp: &[Word]) -> (usize, usize) {
    let join = |ws: &[Word]| ws.iter().map(|w| w.key.as_str()).collect::<Vec<_>>().join(" ").chars().collect::<Vec<_>>();
    let (r, h) = (join(reference), join(hyp));
    (levenshtein(&r, &h), r.len())
}

fn levenshtein<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, x) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(x != y)).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Sustituye en `a` los tramos `diffs` para los que `pick` es verdadero por las
/// palabras de `b` (combinar dos transcripciones del mismo segmento).
pub fn merge(a: &[Word], b: &[Word], diffs: &[Diff], pick: impl Fn(usize) -> bool) -> Vec<Word> {
    let mut out = Vec::with_capacity(a.len());
    let mut i = 0;
    for (n, d) in diffs.iter().enumerate() {
        out.extend_from_slice(&a[i..d.a.start]);
        out.extend_from_slice(if pick(n) { &b[d.b.clone()] } else { &a[d.a.clone()] });
        i = d.a.end;
    }
    out.extend_from_slice(&a[i..]);
    out
}

/// ¿Se tocan dos rangos de índices? Un rango vacío (hueco) toca a otro si cae
/// dentro o justo en su borde.
pub fn touches(x: &Range<usize>, y: &Range<usize>) -> bool {
    if x.is_empty() || y.is_empty() {
        x.start <= y.end && y.start <= x.end
    } else {
        x.start < y.end && y.start < x.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(s: &str) -> Vec<String> {
        words(s).into_iter().map(|w| w.key).collect()
    }

    #[test]
    fn normaliza_puntuacion_tildes_y_mayusculas() {
        assert_eq!(keys("¿Qué tal, Raúl? Año 2025."), ["que", "tal", "raul", "año", "2025"]);
        assert_eq!(keys("— hola …"), ["hola"]);
    }

    #[test]
    fn agrupa_diferencias_consecutivas() {
        let a = words("tenemos muchos camiones en Bilbao");
        let b = words("tenemos mucho camión es en Bilbado");
        let d = diffs(&align(&a, &b));
        assert_eq!(d, [Diff { a: 1..3, b: 1..4 }, Diff { a: 4..5, b: 5..6 }]);
    }

    #[test]
    fn huecos_en_un_lado() {
        let a = words("vale gracias");
        let b = words("vale muchas gracias");
        assert_eq!(diffs(&align(&a, &b)), [Diff { a: 1..1, b: 1..2 }]);
        assert_eq!(edit_errors(&b, &a), 1);
    }

    #[test]
    fn cifras_con_letras_y_con_digitos_coinciden() {
        assert_eq!(keys("treinta y cinco mil setecientos euros"), ["35700", "euros"]);
        assert_eq!(keys("35.700 euros"), ["35700", "euros"]);
        assert_eq!(keys("40 mil"), keys("cuarenta mil"));
        assert_eq!(keys("un diez por ciento"), keys("un 10%"));
        assert_eq!(keys("ciento treinta y cinco"), ["135"]);
        assert_eq!(keys("dos mil veinte"), ["2020"]);
        assert_eq!(keys("dos millones trescientas mil"), ["2300000"]);
        assert_eq!(keys("un almacén y una ruta"), ["un", "almacen", "y", "una", "ruta"]);
        assert_eq!(keys("tenemos tres y luego dos"), ["tenemos", "3", "y", "luego", "2"]);
        assert_eq!(keys("las dos tres veces"), ["las", "2", "3", "veces"]);
        let w = words("unos ochenta y cinco camiones");
        assert_eq!(w[1].text, "ochenta y cinco");
    }

    #[test]
    fn cer_y_mezcla() {
        let r = words("vale gracias");
        assert_eq!(char_errors(&r, &words("vale gracia")), (1, 12));
        let (a, b) = (words("uno dos tres cuatro"), words("uno dós tres cinco seis"));
        let a = words(&a.iter().map(|w| w.text.clone()).collect::<Vec<_>>().join(" ").replace("uno", "hola"));
        let d = diffs(&align(&a, &b));
        let keys = |ws: Vec<Word>| ws.into_iter().map(|w| w.text).collect::<Vec<_>>().join(" ");
        assert_eq!(keys(merge(&a, &b, &d, |_| true)), "uno dos tres cinco seis");
        assert_eq!(keys(merge(&a, &b, &d, |_| false)), "hola dos tres cuatro");
        assert_eq!(keys(merge(&a, &b, &d, |n| n == 1)), "hola dos tres cinco seis");
    }

    #[test]
    fn rangos_que_se_tocan() {
        assert!(touches(&(2..4), &(3..5)));
        assert!(!touches(&(2..4), &(4..6)));
        assert!(touches(&(4..4), &(2..4))); // hueco en el borde
        assert!(touches(&(3..3), &(2..5)));
        assert!(!touches(&(6..6), &(2..4)));
    }
}
