// Idioma de la reunión para el STT (nombres de `support_languages` de
// Qwen3-ASR). Lo comparten la píldora y Ajustes. Cada idioma se muestra en su
// propia lengua; solo «Detectar» sigue el idioma de la interfaz.
import { t, useI18n } from "./i18n";

export const LANGUAGES: [string, string][] = [
  ["Spanish", "Español"],
  ["English", "English"],
  ["auto", "Detect"],
  ["Portuguese", "Português"],
  ["French", "Français"],
  ["German", "Deutsch"],
  ["Italian", "Italiano"],
];
export const languageLabel = ([value, label]: [string, string]) => (value === "auto" ? t("rec.detect") : label);

const LANG_KEY = "juanola.language";

// Sin elegir todavía: el de la interfaz.
const fallback = () => (useI18n.getState().lang === "en" ? "English" : "Spanish");

export const savedLanguage = () => {
  try {
    const l = localStorage.getItem(LANG_KEY);
    return l && LANGUAGES.some(([v]) => v === l) ? l : fallback();
  } catch {
    return fallback();
  }
};

export const saveLanguage = (l: string) => {
  try {
    localStorage.setItem(LANG_KEY, l);
  } catch {
    /* sin almacenamiento: solo dura esta sesión */
  }
};
