//! Idioma fijo para el modo «Detectar».
//!
//! Qwen3-ASR detecta el idioma en cada frase, y en frases cortas o dudosas
//! salta de idioma (en una prueba real, una reunión en español salió en
//! portugués, chino, ruso e hindi). Aquí se suma la duración de las frases
//! finales por idioma detectado y, cuando uno domina con voz suficiente, se fija
//! para el resto de la sesión.

use std::collections::HashMap;

/// Segundos de voz del idioma ganador antes de fijarlo.
const MIN_SECS: f64 = 8.0;
/// Parte de la voz total que debe tener el idioma ganador.
const MIN_SHARE: f64 = 0.6;

#[derive(Debug, Default)]
pub struct LanguageLock {
    secs: HashMap<String, f64>,
    locked: Option<String>,
}

impl LanguageLock {
    /// Idioma fijado, si ya lo hay.
    pub fn locked(&self) -> Option<&str> {
        self.locked.as_deref()
    }

    /// Anota una frase final de `secs` segundos detectada en `language`.
    /// Devuelve el idioma si esta frase lo ha fijado.
    pub fn observe(&mut self, language: &str, secs: f64) -> Option<&str> {
        if self.locked.is_some() || language.is_empty() || secs <= 0.0 {
            return None;
        }
        *self.secs.entry(language.to_owned()).or_default() += secs;
        let total: f64 = self.secs.values().sum();
        let (best, best_secs) = self.secs.iter().max_by(|a, b| a.1.total_cmp(b.1))?;
        if *best_secs >= MIN_SECS && best_secs / total >= MIN_SHARE {
            self.locked = Some(best.clone());
            return self.locked.as_deref();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_dominant_language() {
        let mut l = LanguageLock::default();
        assert_eq!(l.observe("Spanish", 5.0), None);
        assert_eq!(l.observe("Portuguese", 1.0), None);
        assert_eq!(l.observe("Spanish", 4.0), Some("Spanish"));
        assert_eq!(l.locked(), Some("Spanish"));
        // Una vez fijado no cambia.
        assert_eq!(l.observe("Chinese", 50.0), None);
        assert_eq!(l.locked(), Some("Spanish"));
    }

    #[test]
    fn waits_while_mixed() {
        let mut l = LanguageLock::default();
        l.observe("Spanish", 8.0);
        // 8 de 8: se fija enseguida; con mezcla, no.
        let mut m = LanguageLock::default();
        m.observe("Spanish", 6.0);
        m.observe("English", 6.0);
        assert_eq!(m.observe("Spanish", 2.0), None, "8/14 < 60 %");
        assert_eq!(l.locked(), Some("Spanish"));
    }
}
