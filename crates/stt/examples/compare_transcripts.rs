//! Compara transcripciones de una misma sesión frente a la referencia de
//! `tools/script_to_audio.py`: WER y CER por canal de cada una y, con dos,
//! dónde no coinciden, cuántos errores de la primera caen en esos tramos y el
//! mejor combinado posible (techo de cualquier regla que elija tramo a tramo).
//!
//! ```text
//! cargo run --release -p juanola-stt --example compare_transcripts -- <ref.json> <a.json> [b.json]
//! ```
//!
//! `a.json` y `b.json` son salidas de `transcribe_session`.
//! `JUANOLA_SIN_CIFRAS=1` compara sin pasar a dígitos las cifras dichas con letras.

use juanola_core_types::Channel;
use juanola_stt::Transcript;
use juanola_stt::crosscheck::{Word, align, char_errors, diffs, edit_errors, merge, touches, words_with};
use serde::Deserialize;

#[derive(Deserialize)]
struct RefTurn {
    channel: Channel,
    start: f64,
    text: String,
}

fn show(ws: &[Word], r: &std::ops::Range<usize>) -> String {
    if r.is_empty() { "∅".into() } else { ws[r.clone()].iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ") }
}

fn label(path: &str) -> String {
    std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn pct(e: usize, n: usize) -> f64 {
    100.0 * e as f64 / n.max(1) as f64
}

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(a.len() >= 2, "uso: compare_transcripts <ref.json> <a.json> [b.json]");
    let numbers = std::env::var_os("JUANOLA_SIN_CIFRAS").is_none();
    let words = |t: &str| words_with(t, numbers);
    let refs: Vec<RefTurn> = serde_json::from_str(&std::fs::read_to_string(&a[0])?)?;
    let hyps: Vec<(String, Vec<Transcript>)> = a[1..]
        .iter()
        .take(2)
        .map(|p| Ok((label(p), serde_json::from_str(&std::fs::read_to_string(p)?)?)))
        .collect::<anyhow::Result<_>>()?;

    // Por modelo: errores de palabra y de carácter; y del mejor combinado.
    let n = hyps.len();
    let (mut nw, mut nc) = (0, 0);
    let mut we = vec![0usize; n + 1];
    let mut ce = vec![0usize; n + 1];
    let (mut tot_err, mut tot_hit, mut tot_flags, mut tot_flag_ok) = (0, 0, 0, 0);
    for ch in [Channel::Mic, Channel::System] {
        let name = if ch == Channel::Mic { "mic" } else { "system" };
        let mut r: Vec<&RefTurn> = refs.iter().filter(|x| x.channel == ch).collect();
        r.sort_by(|x, y| x.start.total_cmp(&y.start));
        let rw: Vec<Word> = r.iter().flat_map(|x| words(&x.text)).collect();
        let hw: Vec<Vec<Word>> = hyps
            .iter()
            .map(|(_, segs)| {
                let mut s: Vec<&Transcript> = segs.iter().filter(|t| t.channel == ch && t.is_final).collect();
                s.sort_by(|x, y| x.start.total_cmp(&y.start));
                s.iter().flat_map(|t| words(&t.text)).collect()
            })
            .collect();
        let n_chars = char_errors(&rw, &[]).1;
        let mut line = format!("{name:<6} {} palabras ·", rw.len());
        for (k, h) in hw.iter().enumerate() {
            let (w, c) = (edit_errors(&rw, h), char_errors(&rw, h).0);
            we[k] += w;
            ce[k] += c;
            line += &format!(" {} {:.1}/{:.1} %", hyps[k].0, pct(w, rw.len()), pct(c, n_chars));
        }
        if let [x, y] = &hw[..] {
            let d = diffs(&align(x, y));
            // Mejor combinado: empezando por `a`, cada tramo distinto se cambia
            // por el de `b` si eso baja los errores frente a la referencia.
            let mut picks = vec![false; d.len()];
            let mut best = edit_errors(&rw, x);
            for i in 0..d.len() {
                picks[i] = true;
                let e = edit_errors(&rw, &merge(x, y, &d, |k| picks[k]));
                if e < best { best = e } else { picks[i] = false }
            }
            let ow = merge(x, y, &d, |k| picks[k]);
            we[n] += best;
            ce[n] += char_errors(&rw, &ow).0;
            let flags: Vec<_> = d.iter().map(|d| d.a.clone()).collect();
            let errors: Vec<_> = diffs(&align(&rw, x)).into_iter().map(|d| d.b).collect();
            let hit = errors.iter().filter(|e| flags.iter().any(|f| touches(e, f))).count();
            let ok = flags.iter().filter(|f| errors.iter().any(|e| touches(e, f))).count();
            line += &format!(
                " · mejor combinado {:.1}/{:.1} % · errores de {} en tramos distintos {hit}/{} · tramos con error {ok}/{}",
                pct(best, rw.len()),
                pct(char_errors(&rw, &ow).0, n_chars),
                hyps[0].0,
                errors.len(),
                d.len()
            );
            tot_err += errors.len();
            tot_hit += hit;
            tot_flags += d.len();
            tot_flag_ok += ok;
            println!("{line}");
            println!("       tramos distintos ({} → {}):", hyps[0].0, hyps[1].0);
            for t in &d {
                println!("         «{}» → «{}»", show(x, &t.a), show(y, &t.b));
            }
        } else {
            println!("{line}");
        }
        nw += rw.len();
        nc += n_chars;
    }
    let mut line = "TOTAL  WER/CER".to_owned();
    for k in 0..n {
        line += &format!(" · {} {:.1}/{:.1} %", hyps[k].0, pct(we[k], nw), pct(ce[k], nc));
    }
    if n == 2 {
        line += &format!(
            " · mejor combinado {:.1}/{:.1} % · recall {:.0} % ({tot_hit}/{tot_err}) · precisión {:.0} % ({tot_flag_ok}/{tot_flags})",
            pct(we[n], nw),
            pct(ce[n], nc),
            pct(tot_hit, tot_err),
            pct(tot_flag_ok, tot_flags)
        );
    }
    println!("{line}");
    Ok(())
}
