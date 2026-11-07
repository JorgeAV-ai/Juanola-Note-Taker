import { useEffect, useRef, useState } from "react";
import { events, ipc, type Note } from "../lib/ipc";
import { useTranscript } from "../state/transcript";
import { EnhancedView } from "./EnhancedView";
import { NoteEditor } from "./NoteEditor";
import { RecordingPill } from "./RecordingPill";

type Tab = "mine" | "enhanced";

// Vista de nota: título inline, pestañas My notes / Enhanced, píldora de grabación.
// Al parar la grabación se generan las notas mejoradas automáticamente.
export function NoteView({ id }: { id: string }) {
  const [note, setNote] = useState<Note | null>(null);
  const [tab, setTab] = useState<Tab>("mine");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = useTranscript((s) => s.load);
  const titleTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    setNote(null);
    ipc.getNote(id).then((n) => {
      setNote(n);
      if (n?.enhanced_doc) setTab("enhanced");
    });
    load(id);
    const un = events.noteStatus((e) => {
      if (e.note_id !== id) return;
      setBusy(e.status === "enhancing" ? "Enhancing notes…" : null);
      if (e.status === "failed") setError(e.message ?? "Falló el enhance");
    });
    return () => {
      un.then((f) => f());
    };
  }, [id, load]);

  const enhanceNow = async () => {
    setError(null);
    setBusy("Enhancing notes…");
    try {
      const enhanced = await ipc.enhanceNote(id);
      setNote((n) => (n ? { ...n, enhanced_doc: enhanced, status: "ready" } : n));
      setTab("enhanced");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  if (!note) return <div className="note" />;
  return (
    <div className="note" data-note-id={id}>
      <input
        className="note-title"
        placeholder="Untitled"
        defaultValue={note.title}
        onChange={(e) => {
          const t = e.target.value;
          window.clearTimeout(titleTimer.current);
          titleTimer.current = window.setTimeout(() => ipc.setNoteTitle(id, t), 400);
        }}
      />
      <div className="note-tabs">
        <button className={`tab ${tab === "mine" ? "active" : ""}`} onClick={() => setTab("mine")}>My notes</button>
        <button className={`tab ${tab === "enhanced" ? "active" : ""}`} onClick={() => setTab("enhanced")} disabled={!note.enhanced_doc}>
          Enhanced
        </button>
        <span className="spacer" />
        {busy && <span className="status">{busy}</span>}
        <button className="icon-btn" title="Generate notes" onClick={enhanceNow} disabled={!!busy}>✨</button>
      </div>
      {error && <div className="note-error">{error}</div>}
      {tab === "mine" || !note.enhanced_doc ? (
        <NoteEditor key={id} initial={note.raw_doc} onChange={(doc) => ipc.setNoteRawDoc(id, doc)} />
      ) : (
        <EnhancedView notes={note.enhanced_doc} />
      )}
      <RecordingPill noteId={id} onStopped={(saved) => (saved > 0 ? enhanceNow() : setError("No se ha transcrito nada."))} />
    </div>
  );
}
