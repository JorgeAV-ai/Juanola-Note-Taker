// Idioma de la interfaz (castellano o inglés). Distinto del idioma de las
// reuniones (lib/language.ts), que decide cómo se transcribe.
//
// `t()` lee el idioma actual; los componentes llaman a `useT()` para volver a
// pintarse cuando cambia. Se guarda en localStorage, que comparten la ventana
// principal y la del aviso de reunión (el evento `storage` avisa a la otra).
import { create } from "zustand";
import { en, es, type Key } from "./strings";

export type UiLang = "es" | "en";
export type UiPref = "system" | UiLang;

const KEY = "juanola.uiLanguage";
const DICTS: Record<UiLang, Record<Key, string>> = { en, es };

/** El primer idioma preferido del sistema que sea castellano o inglés; si no, inglés. */
export function systemLang(): UiLang {
  const prefs = navigator.languages?.length ? navigator.languages : [navigator.language];
  for (const p of prefs) {
    const base = p.toLowerCase().split("-")[0];
    if (base === "es" || base === "en") return base;
  }
  return "en";
}

function savedPref(): UiPref {
  try {
    const v = localStorage.getItem(KEY);
    return v === "es" || v === "en" ? v : "system";
  } catch {
    return "system";
  }
}

const resolve = (p: UiPref): UiLang => (p === "system" ? systemLang() : p);

type State = { pref: UiPref; lang: UiLang; setPref: (p: UiPref, persist?: boolean) => void };

export const useI18n = create<State>((set) => ({
  pref: savedPref(),
  lang: resolve(savedPref()),
  setPref: (pref, persist = true) => {
    if (persist) {
      try {
        if (pref === "system") localStorage.removeItem(KEY);
        else localStorage.setItem(KEY, pref);
      } catch {
        /* sin almacenamiento: solo dura esta sesión */
      }
    }
    set({ pref, lang: resolve(pref) });
    document.documentElement.lang = resolve(pref);
  },
}));

document.documentElement.lang = useI18n.getState().lang;
window.addEventListener("storage", (e) => {
  if (e.key === KEY || e.key === null) useI18n.getState().setPref(savedPref(), false);
});

/** Texto de `key` en el idioma actual, con `{var}` sustituidas. */
export function t(key: Key, vars?: Record<string, string | number>): string {
  let s = DICTS[useI18n.getState().lang][key] ?? en[key];
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

/** Suscribe el componente al idioma y devuelve `t`. */
export function useT() {
  useI18n((s) => s.lang);
  return t;
}

/** Locale para fechas y horas. */
export const dateLocale = () => (useI18n.getState().lang === "es" ? "es-ES" : "en-GB");

// Mensajes que el backend manda ya escritos (en castellano). Se reconocen por
// su texto exacto y se traducen; los errores técnicos pasan tal cual.
const BACKEND: Record<string, Key> = {
  [es["backend.modelsMissing"]]: "backend.modelsMissing",
  [es["backend.stopBeforeDelete"]]: "backend.stopBeforeDelete",
  [es["backend.micAsk"]]: "backend.micAsk",
  [es["backend.micSilent"]]: "backend.micSilent",
  [es["backend.systemSilent"]]: "backend.systemSilent",
  [es["backend.testMeeting"]]: "backend.testMeeting",
  [es["backend.stillStopping"]]: "backend.stillStopping",
  [es["backend.noteTrashed"]]: "backend.noteTrashed",
  [es["backend.noteMissing"]]: "backend.noteMissing",
  [es["backend.badEvent"]]: "backend.badEvent",
  // Estos dos dicen «Start» en el backend; en `es` se muestran con «Empezar».
  "Juanola no tiene permiso para usar el micrófono. Actívalo en Ajustes del Sistema → Privacidad y seguridad → Micrófono y vuelve a pulsar Start.": "backend.micDenied",
  "No se respondió al permiso de micrófono. Vuelve a pulsar Start o actívalo en Ajustes del Sistema.": "backend.micUnanswered",
};

export function tBackend(message: string | null | undefined): string {
  if (!message) return "";
  const key = BACKEND[message.replace(/^Error: /, "")];
  return key ? t(key) : message;
}
