import { useEffect, useState } from "react";
import { ipc, type CalendarEvent, type Note } from "../lib/ipc";

// Home de Granola: "Coming up" (5 a la vez, paginado) + lista cronológica de notas en filas.
export function Home({ onOpenNote }: { onOpenNote: (id: string) => void }) {
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [notes, setNotes] = useState<Note[]>([]);

  useEffect(() => {
    ipc.comingUp().then(setEvents).catch(() => setEvents([]));
    ipc.listNotes().then(setNotes).catch(() => setNotes([]));
  }, []);

  const newNote = async () => {
    const n = await ipc.createNote();
    onOpenNote(n.id);
  };

  return (
    <div className="home">
      <section>
        <h2 className="section-title">Coming up</h2>
        {events.length === 0 ? (
          <p className="empty">Conecta tu calendario en Settings para ver las próximas reuniones.</p>
        ) : (
          <ul className="rows">
            {events.slice(0, 5).map((e) => (
              <li key={e.id} className="row">
                <span className="row-title">{e.title}</span>
                <span className="row-meta">{new Date(e.start).toLocaleTimeString()}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section>
        <div className="section-head">
          <h2 className="section-title">Your notes</h2>
          <button className="primary" onClick={newNote}>＋ New note</button>
        </div>
        {notes.length === 0 ? (
          <p className="empty">Todavía no hay notas.</p>
        ) : (
          <ul className="rows">
            {notes.map((n) => (
              <li key={n.id} className="row" onClick={() => onOpenNote(n.id)}>
                <span className="row-title">{n.title || "Untitled"}</span>
                <span className="row-meta">{new Date(n.created_at).toLocaleDateString()}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
