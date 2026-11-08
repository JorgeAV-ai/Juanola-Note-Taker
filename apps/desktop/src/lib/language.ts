// Idioma de la reunión para el STT (nombres de `support_languages` de
// Qwen3-ASR). Lo comparten la píldora y Ajustes.
export const LANGUAGES: [string, string][] = [
  ["Spanish", "Español"],
  ["English", "English"],
  ["auto", "Detectar"],
  ["Portuguese", "Português"],
  ["French", "Français"],
  ["German", "Deutsch"],
  ["Italian", "Italiano"],
];
const LANG_KEY = "juanola.language";

export const savedLanguage = () => {
  try {
    const l = localStorage.getItem(LANG_KEY);
    return l && LANGUAGES.some(([v]) => v === l) ? l : "Spanish";
  } catch {
    return "Spanish";
  }
};

export const saveLanguage = (l: string) => {
  try {
    localStorage.setItem(LANG_KEY, l);
  } catch {
    /* sin almacenamiento: solo dura esta sesión */
  }
};
