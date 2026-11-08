import { useEffect, useRef, useState, type ReactNode } from "react";
import { ChevronDown, ChevronRight, ChevronUp, Copy, Languages, Mic, Minus, Pause, Play, Search, Square, X } from "lucide-react";
import { events, ipc, MODELS_MISSING, type RecordingState, type SettingsAction } from "@/lib/ipc";
import { useModels } from "@/state/models";
import { LANGUAGES, saveLanguage, savedLanguage } from "@/lib/language";
import { cn } from "@/lib/utils";
import { useTranscript } from "@/state/transcript";

// Zona inferior de la nota, como en Granola. Plegada: círculo con las barras
// (y parar/pausa mientras graba) a la izquierda de «Ask anything». Desplegada:
// una tarjeta con el transcript (Them a la izquierda, Me a la derecha) y, en su
// pie, las barras, parar, el micrófono y el idioma; tapa «Ask anything».
// Nonces de `autostart` ya atendidos (StrictMode monta los efectos dos veces).
const autostarted = new Set<number>();

export function RecordingPill({ noteId, autostart, onStopped, askBar }: { noteId: string; autostart?: number; onStopped: (saved: number) => void; askBar: ReactNode }) {
  const [state, setState] = useState<RecordingState | "idle">("idle");
  const [message, setMessage] = useState<string | null>(null);
  const [action, setAction] = useState<SettingsAction | null>(null);
  const [level, setLevel] = useState(0);
  const [open, setOpen] = useState(false);
  const [language, setLanguage] = useState(savedLanguage);
  const [mic, setMic] = useState<string | null>(null);
  const [query, setQuery] = useState<string | null>(null);
  const lines = useTranscript((s) => s.lines);
  const apply = useTranscript((s) => s.apply);
  const bottom = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const subs = [
      events.recordingState((e) => {
        if (e.note_id !== noteId) return;
        if (e.state === "warning") {
          setMessage(e.message ?? null);
          setAction(e.action ?? null);
          return;
        }
        setState(e.state);
        // Un cambio de estado normal no borra un aviso pendiente.
        if (e.message || e.state === "error" || e.state === "starting") {
          setMessage(e.message ?? null);
          setAction(e.action ?? null);
        }
      }),
      events.levels((l) => setLevel(Math.max(l.mic, l.system))),
      events.segment(apply),
    ];
    return () => subs.forEach((p) => p.then((un) => un()));
  }, [noteId, apply]);

  useEffect(() => {
    if (!open) return;
    ipc
      .listAudioDevices()
      .then((d) => setMic(d.find((x) => x.is_default)?.name ?? null))
      .catch(() => setMic(null));
  }, [open]);

  // Con llaves: en navegadores recientes scrollIntoView devuelve una promesa,
  // y un efecto solo puede devolver su función de limpieza.
  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [lines.length, open]);

  const live = state === "recording" || state === "starting";
  const start = async () => {
    setMessage(null);
    setAction(null);
    setOpen(true);
    try {
      await ipc.startRecording(noteId, language);
    } catch (e) {
      if (String(e) === MODELS_MISSING) {
        setOpen(false);
        useModels.getState().openDialog(true);
      }
      setState("error");
      setMessage(String(e));
    }
  };
  const stop = async () => {
    setState("finishing");
    try {
      const saved = await ipc.stopRecording();
      onStopped(saved);
    } catch (e) {
      setState("error");
      setMessage(String(e));
    }
  };

  // Desde un aviso de reunión: empezar a grabar al abrir la nota.
  useEffect(() => {
    if (!autostart || autostarted.has(autostart)) return;
    autostarted.add(autostart);
    start();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autostart]);

  // La app de la llamada soltó el micrófono: la reunión terminó.
  // Refs: el aviso puede llegar mucho después de montar, con `stop` y su
  // `onStopped` ya renovados.
  const stateRef = useRef(state);
  stateRef.current = state;
  const stopRef = useRef(stop);
  stopRef.current = stop;
  useEffect(() => {
    const un = events.meetingEnded(() => {
      if (stateRef.current === "recording" || stateRef.current === "paused") stopRef.current();
    });
    return () => {
      un.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [noteId]);

  // Barras: altura según el nivel (RMS → dB, -60..0).
  const db = 20 * Math.log10(Math.max(level, 1e-6));
  const h = Math.max(0, Math.min(1, (db + 60) / 60));
  const label = state === "starting" ? "Preparando…" : state === "finishing" ? "Transcribiendo…" : state === "paused" ? "En pausa" : null;

  const bars = (
    <span className={cn("bars", state === "recording" ? "level" : state === "paused" ? "idle" : live ? "" : "idle")}>
      {[0.55, 1, 0.7, 0.9].map((k, i) => (
        <i key={i} style={state === "recording" ? { height: `${6 + 10 * h * k}px` } : { height: `${[9, 15, 11, 13][i]}px` }} />
      ))}
    </span>
  );

  const toggle = (
    <button
      className="flex h-11 items-center gap-1.5 rounded-full pl-3 pr-2 hover:bg-hover"
      title="Transcript"
      aria-label="Transcript"
      aria-expanded={open}
      onClick={() => setOpen((o) => !o)}
    >
      {(state === "recording" || state === "paused") && <span className={cn("rec-diamond", state === "paused" && "paused")} title={state === "recording" ? "Grabando" : "En pausa"} />}
      {bars}
      {open ? <ChevronDown className="size-4 text-ink-2" /> : <ChevronUp className="size-4 text-ink-2" />}
    </button>
  );

  const controls =
    state === "recording" || state === "paused" ? (
      <>
        {state === "recording" ? (
          <button className="pill-btn flex size-10 items-center justify-center rounded-full text-ink-2 hover:bg-hover" title="Pausar" onClick={() => ipc.pauseRecording()}>
            <Pause className="size-[17px]" fill="currentColor" strokeWidth={0} />
            <span className="sr-only">Pause</span>
          </button>
        ) : (
          <button className="pill-btn flex size-10 items-center justify-center rounded-full text-olive-ink hover:bg-hover" title="Reanudar" onClick={() => ipc.resumeRecording()}>
            <Play className="size-[17px]" fill="currentColor" strokeWidth={0} />
            <span className="sr-only">Resume</span>
          </button>
        )}
        <button className="pill-btn stop flex size-10 items-center justify-center rounded-full text-ink-2 hover:bg-hover" title="Parar" onClick={stop}>
          <Square className="size-4" fill="currentColor" strokeWidth={0} />
          <span className="sr-only">Stop</span>
        </button>
      </>
    ) : state === "starting" ? (
      <button className="pill-btn stop flex size-10 items-center justify-center rounded-full text-ink-2 hover:bg-hover" title="Parar" onClick={stop}>
        <Square className="size-4" fill="currentColor" strokeWidth={0} />
        <span className="sr-only">Stop</span>
      </button>
    ) : state === "finishing" ? null : (
      <button className="pill-btn h-10 rounded-full px-3.5 text-[15px] font-medium text-olive-ink hover:bg-hover" onClick={start}>
        {lines.length ? "Resume" : "Start"}
      </button>
    );

  const notice = message && (
    <div
      className={cn(
        "pill-msg rounded-2xl border border-line bg-surface px-4 py-3 text-[13.5px] text-ink-2",
        state === "error" && "error border-[#f1d3a8] bg-[#fdf6ea] text-[#8a4b0f] dark:border-[#6b4a1e] dark:bg-[#3a2e1c] dark:text-[#f3c98b]",
      )}
    >
      {message}
      {action && (
        <button
          className="pill-msg-btn ml-2 inline-flex h-7 items-center rounded-full border border-line bg-surface px-3 text-[13px] text-ink hover:bg-hover"
          onClick={() => ipc.openPrivacySettings(action).catch((e) => setMessage(String(e)))}
        >
          Abrir Ajustes
        </button>
      )}
    </div>
  );

  if (!open) {
    return (
      <div>
        {notice && <div className="mb-3 w-[min(560px,100%)]">{notice}</div>}
        <div className="flex items-end gap-3">
          <div className="pill flex h-[58px] shrink-0 items-center gap-0.5 rounded-full border border-line bg-surface px-1.5 shadow-[0_2px_12px_rgba(0,0,0,0.04)]">
            {toggle}
            {label && <span className="pill-label px-1 text-[13px] text-muted">{label}</span>}
            {controls}
          </div>
          <div className="min-w-0 flex-1">{askBar}</div>
        </div>
      </div>
    );
  }

  const shown = query ? lines.filter((l) => l.text.toLowerCase().includes(query.toLowerCase())) : lines;
  return (
    <div className="transcript-drawer flex max-h-[min(62vh,620px)] flex-col overflow-hidden rounded-[28px] border border-line bg-surface shadow-[0_12px_40px_rgba(0,0,0,0.10)]">
      <div className="flex h-12 shrink-0 items-center gap-1 px-3">
        {query === null ? (
          <button className="flex size-8 items-center justify-center rounded-full text-ink-2 hover:bg-hover" title="Buscar en el transcript" onClick={() => setQuery("")}>
            <Search className="size-[17px]" />
          </button>
        ) : (
          <div className="flex h-8 flex-1 items-center gap-2 rounded-full bg-hover px-3">
            <Search className="size-4 text-muted" />
            <input autoFocus className="min-w-0 flex-1 bg-transparent text-[14px] outline-none" placeholder="Buscar en el transcript" value={query} onChange={(e) => setQuery(e.target.value)} />
            <button className="text-muted hover:text-ink" aria-label="Cerrar búsqueda" onClick={() => setQuery(null)}>
              <X className="size-4" />
            </button>
          </div>
        )}
        <span className="flex-1" />
        <button
          className="flex size-8 items-center justify-center rounded-full text-ink-2 hover:bg-hover"
          title="Copy all"
          onClick={() => navigator.clipboard.writeText(lines.map((l) => `${l.channel === "mic" ? "Me" : "Them"}: ${l.text}`).join("\n"))}
        >
          <Copy className="size-[17px]" />
        </button>
        <button className="flex size-8 items-center justify-center rounded-full text-ink-2 hover:bg-hover" title="Minimizar" aria-label="Minimizar" onClick={() => setOpen(false)}>
          <Minus className="size-[17px]" />
        </button>
      </div>

      <div className="bubbles flex min-h-[200px] flex-1 flex-col gap-2 overflow-auto px-5 pb-4">
        <p className="mb-2 flex items-center justify-center gap-1 text-center text-[13px] text-muted">
          Pide siempre permiso antes de transcribir a otros <ChevronRight className="size-3.5" />
        </p>
        {notice}
        {shown.length === 0 && (
          <p className="empty py-6 text-center text-[14px] text-muted">{query ? "Nada coincide." : live ? "Escuchando…" : "Sin transcripción todavía."}</p>
        )}
        {shown.map((l) => (
          <div
            key={l.id}
            className={cn(
              "bubble max-w-[78%] rounded-xl px-3 py-1.5 text-[14.5px] leading-snug",
              l.channel === "mic" ? "me self-end bg-[var(--bubble-me)]" : "them self-start bg-[var(--bubble-them)]",
              !l.final && "partial text-ink-2",
            )}
          >
            {l.text}
            {!l.final && <span className="typing ml-1.5 inline-flex gap-0.5 align-middle"><i /><i /><i /></span>}
          </div>
        ))}
        <div ref={bottom} />
      </div>

      <div className="pill flex h-[60px] shrink-0 items-center gap-1 border-t border-line px-3">
        {toggle}
        {label && <span className="pill-label px-1 text-[13px] text-muted">{label}</span>}
        {controls}
        <span className="flex-1" />
        {mic && (
          <span className="hidden items-center gap-1.5 px-2 text-[14px] text-ink-2 md:flex" title="Juanola graba del micrófono predeterminado (se cambia en Ajustes del Sistema › Sonido)">
            <Mic className="size-4" /> {mic}
          </span>
        )}
        <label className="relative flex h-9 cursor-pointer items-center gap-1.5 rounded-full px-2.5 text-[14px] text-ink-2 hover:bg-hover" title="Idioma de la reunión">
          <Languages className="size-4" />
          {LANGUAGES.find(([v]) => v === language)?.[1]}
          <ChevronDown className="size-3.5" />
          {/* El select invisible encima: el texto visible ocupa lo justo. */}
          <select
            className="pill-lang absolute inset-0 cursor-pointer opacity-0 disabled:cursor-default"
            value={language}
            disabled={live || state === "paused"}
            onChange={(e) => {
              setLanguage(e.target.value);
              saveLanguage(e.target.value);
            }}
          >
            {LANGUAGES.map(([v, l]) => (
              <option key={v} value={v}>
                {l}
              </option>
            ))}
          </select>
        </label>
      </div>
    </div>
  );
}
