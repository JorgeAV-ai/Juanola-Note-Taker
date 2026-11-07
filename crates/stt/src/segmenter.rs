//! Segmentación de voz en streaming a partir de la probabilidad de voz del VAD.
//!
//! Qwen3-ASR transcribe tramos completos, no flujo continuo. Para dar texto en
//! vivo se usa el patrón de "streaming simulado": mientras dura una frase se
//! re-transcribe el tramo acumulado cada `partial_every` (resultado parcial,
//! `is_final = false`) y, cuando el VAD detecta un silencio de
//! `min_silence`, se transcribe por última vez (resultado final). Este módulo
//! solo decide *qué tramo* transcribir y *cuándo*; no sabe nada del modelo.

/// Parámetros en muestras a 16 kHz salvo indicación.
#[derive(Debug, Clone)]
pub struct SegmenterConfig {
    /// Probabilidad a partir de la cual empieza la voz.
    pub start_threshold: f32,
    /// Probabilidad por debajo de la cual cuenta como silencio (histéresis).
    pub end_threshold: f32,
    /// Silencio continuo que cierra una frase.
    pub min_silence: u64,
    /// Audio previo al inicio detectado que se incluye (el VAD llega tarde).
    pub pre_roll: u64,
    /// Margen tras el último tramo de voz al cerrar.
    pub post_roll: u64,
    /// Cadencia de los resultados parciales.
    pub partial_every: u64,
    /// Duración máxima de una frase; se corta y sigue otra.
    pub max_utterance: u64,
    /// Frases con menos voz que esto se descartan (clics, golpes).
    pub min_speech: u64,
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        let ms = |v: u64| v * 16;
        Self {
            start_threshold: 0.5,
            end_threshold: 0.35,
            min_silence: ms(600),
            pre_roll: ms(300),
            post_roll: ms(150),
            partial_every: ms(1000),
            max_utterance: ms(20_000),
            min_speech: ms(250),
        }
    }
}

/// Tramo de audio a transcribir, en muestras absolutas del canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Partial(Span),
    Final(Span),
}

#[derive(Debug)]
enum State {
    Idle,
    Speech {
        start: u64,
        last_voice: u64,
        last_partial: u64,
        voiced: u64,
    },
}

#[derive(Debug)]
pub struct Segmenter {
    cfg: SegmenterConfig,
    state: State,
}

impl Segmenter {
    pub fn new(cfg: SegmenterConfig) -> Self {
        Self { cfg, state: State::Idle }
    }

    /// `true` mientras hay una frase abierta.
    pub fn in_speech(&self) -> bool {
        matches!(self.state, State::Speech { .. })
    }

    /// Primera muestra que aún puede formar parte de un tramo; lo anterior se
    /// puede descartar del búfer de audio.
    pub fn keep_from(&self, now: u64) -> u64 {
        match self.state {
            State::Idle => now.saturating_sub(self.cfg.pre_roll),
            State::Speech { start, .. } => start,
        }
    }

    /// Procesa la probabilidad de voz de la ventana `[end - len, end)`.
    pub fn push(&mut self, prob: f32, len: u64, end: u64) -> Option<Action> {
        let c = &self.cfg;
        let begin = end.saturating_sub(len);
        match &mut self.state {
            State::Idle => {
                if prob >= c.start_threshold {
                    self.state = State::Speech {
                        start: begin.saturating_sub(c.pre_roll),
                        last_voice: end,
                        last_partial: end,
                        voiced: len,
                    };
                }
                None
            }
            State::Speech { start, last_voice, last_partial, voiced } => {
                if prob >= c.end_threshold {
                    *last_voice = end;
                    *voiced += len;
                }
                if end - *last_voice >= c.min_silence {
                    let span = Span { start: *start, end: (*last_voice + c.post_roll).min(end) };
                    let keep = *voiced >= c.min_speech;
                    self.state = State::Idle;
                    return keep.then_some(Action::Final(span));
                }
                if end - *start >= c.max_utterance {
                    let span = Span { start: *start, end };
                    *start = end;
                    *last_partial = end;
                    *voiced = 0;
                    return Some(Action::Final(span));
                }
                if end - *last_partial >= c.partial_every {
                    *last_partial = end;
                    return Some(Action::Partial(Span { start: *start, end }));
                }
                None
            }
        }
    }

    /// Cierra la frase abierta, si la hay (fin de la grabación).
    pub fn flush(&mut self, now: u64) -> Option<Action> {
        let State::Speech { start, voiced, .. } = self.state else { return None };
        self.state = State::Idle;
        (voiced >= self.cfg.min_speech).then_some(Action::Final(Span { start, end: now }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u64 = 512;

    fn run(probs: &[f32]) -> Vec<Action> {
        let mut s = Segmenter::new(SegmenterConfig::default());
        let mut out = Vec::new();
        for (i, p) in probs.iter().enumerate() {
            if let Some(a) = s.push(*p, W, (i as u64 + 1) * W) {
                out.push(a);
            }
        }
        out.extend(s.flush(probs.len() as u64 * W));
        out
    }

    #[test]
    fn silence_produces_nothing() {
        assert!(run(&[0.0; 200]).is_empty());
    }

    #[test]
    fn short_click_is_dropped() {
        let mut p = vec![0.0; 10];
        p.push(0.9); // 32 ms de "voz"
        p.extend([0.0; 60]);
        assert!(run(&p).is_empty());
    }

    #[test]
    fn utterance_gives_partials_then_final() {
        // 10 ventanas de silencio, 2,5 s de voz (~78 ventanas), 1 s de silencio.
        let mut p = vec![0.0; 10];
        p.extend(vec![0.9; 78]);
        p.extend(vec![0.0; 32]);
        let acts = run(&p);
        let partials = acts.iter().filter(|a| matches!(a, Action::Partial(_))).count();
        assert_eq!(partials, 2, "{acts:?}");
        let Action::Final(f) = acts.last().unwrap() else { panic!("{acts:?}") };
        let pre = SegmenterConfig::default().pre_roll;
        assert_eq!(f.start, 10 * W - pre);
        assert_eq!(f.end, 88 * W + SegmenterConfig::default().post_roll);
    }

    #[test]
    fn long_speech_is_cut_at_max_utterance() {
        let acts = run(&vec![0.9; 1300]); // ~41 s
        let finals = acts.iter().filter(|a| matches!(a, Action::Final(_))).count();
        assert_eq!(finals, 3, "dos cortes a 20 s + el flush");
    }

    #[test]
    fn hysteresis_keeps_speech_through_dips() {
        // Bajadas a 0.4 (entre umbrales) no cierran la frase.
        let mut p = vec![0.9; 20];
        p.extend(vec![0.4; 40]);
        p.extend(vec![0.9; 20]);
        p.extend(vec![0.0; 40]);
        let finals = run(&p).into_iter().filter(|a| matches!(a, Action::Final(_))).count();
        assert_eq!(finals, 1);
    }
}
