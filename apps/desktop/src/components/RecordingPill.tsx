import { useEffect, useRef, useState } from "react";
import { events, ipc, type RecordingState } from "../lib/ipc";
import { useTranscript } from "../state/transcript";

// Píldora abajo-centro: barras con el nivel real, Start / Stop / Resume, y el
// drawer con el transcript en vivo (Them a la izquierda, Me a la derecha).
export function RecordingPill({ noteId, onStopped }: { noteId: string; onStopped: (saved: number) => void }) {
  const [state, setState] = useState<RecordingState | "idle">("idle");
  const [message, setMessage] = useState<string | null>(null);
  const [level, setLevel] = useState(0);
  const [open, setOpen] = useState(false);
  const lines = useTranscript((s) => s.lines);
  const apply = useTranscript((s) => s.apply);
  const bottom = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const subs = [
      events.recordingState((e) => {
        if (e.note_id !== noteId) return;
        if (e.state === "warning") {
          setMessage(e.message ?? null);
          return;
        }
        setState(e.state);
        setMessage(e.message ?? null);
      }),
      events.levels((l) => setLevel(Math.max(l.mic, l.system))),
      events.segment(apply),
    ];
    return () => subs.forEach((p) => p.then((un) => un()));
  }, [noteId, apply]);

  useEffect(() => bottom.current?.scrollIntoView({ block: "end" }), [lines.length, open]);

  const live = state === "recording" || state === "starting";
  const start = async () => {
    setMessage(null);
    setOpen(true);
    try {
      await ipc.startRecording(noteId);
    } catch (e) {
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

  // Barras: altura según el nivel (RMS → dB, -60..0).
  const db = 20 * Math.log10(Math.max(level, 1e-6));
  const h = Math.max(0, Math.min(1, (db + 60) / 60));
  const label =
    state === "starting" ? "Preparando…" : state === "finishing" ? "Transcribiendo…" : state === "paused" ? "En pausa" : null;

  return (
    <>
      {open && (
        <div className="transcript-drawer">
          <div className="drawer-head">
            <span>Transcript</span>
            <button
              className="icon-btn"
              title="Copy all"
              onClick={() =>
                navigator.clipboard.writeText(lines.map((l) => `${l.channel === "mic" ? "Me" : "Them"}: ${l.text}`).join("\n"))
              }
            >
              ⧉
            </button>
          </div>
          <div className="bubbles">
            {lines.length === 0 && <p className="empty">{live ? "Escuchando…" : "Sin transcripción todavía."}</p>}
            {lines.map((l) => (
              <div key={l.id} className={`bubble ${l.channel === "mic" ? "me" : "them"} ${l.final ? "" : "partial"}`}>
                {l.text}
              </div>
            ))}
            <div ref={bottom} />
          </div>
        </div>
      )}
      <div className="pill">
        <button className="icon-btn" title="Transcript" onClick={() => setOpen((o) => !o)}>
          ≋
        </button>
        <div className={`bars ${state === "recording" ? "level" : ""}`}>
          {[0.6, 0.9, 1, 0.8, 0.5].map((k, i) => (
            <i key={i} style={state === "recording" ? { height: `${4 + 12 * h * k}px` } : undefined} />
          ))}
        </div>
        {label && <span className="pill-label">{label}</span>}
        {state === "recording" || state === "paused" ? (
          <>
            {state === "recording" ? (
              <button className="pill-btn" onClick={() => ipc.pauseRecording()}>Pause</button>
            ) : (
              <button className="pill-btn" onClick={() => ipc.resumeRecording()}>Resume</button>
            )}
            <button className="pill-btn stop" onClick={stop}>Stop</button>
          </>
        ) : state === "starting" || state === "finishing" ? null : (
          <button className="pill-btn" onClick={start}>{lines.length ? "Resume" : "Start"}</button>
        )}
      </div>
      {message && <div className={`pill-msg ${state === "error" ? "error" : ""}`}>{message}</div>}
    </>
  );
}
