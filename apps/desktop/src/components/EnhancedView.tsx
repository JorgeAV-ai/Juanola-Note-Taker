import { useState } from "react";
import type { EnhancedNotes } from "../lib/ipc";
import { excerpt, useTranscript } from "../state/transcript";

const clock = (s: number) => `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

// Notas mejoradas: texto del usuario en negro, texto de la IA en gris; la lupa
// muestra el fragmento de la transcripción que respalda cada viñeta.
export function EnhancedView({ notes }: { notes: EnhancedNotes }) {
  const lines = useTranscript((s) => s.lines);
  const [open, setOpen] = useState<string | null>(null);
  return (
    <div className="enhanced">
      {notes.sections.map((s, si) => (
        <section key={si}>
          <h3>{s.heading}</h3>
          <ul>
            {s.bullets.map((b, bi) => {
              const key = `${si}-${bi}`;
              const src = b.source?.kind === "transcript" ? b.source : null;
              return (
                <li key={key} className={`${b.is_user_text ? "user-text" : "ai-text"} ${b.is_action_item ? "task" : ""}`}>
                  {b.is_action_item && <span className="task-box">☐</span>}
                  <span>{b.text}</span>
                  {src && (
                    <button className="cite" title="Ver en la transcripción" onClick={() => setOpen(open === key ? null : key)}>
                      ⌕
                    </button>
                  )}
                  {src && open === key && (
                    <div className="cite-pop">
                      <div className="cite-time">
                        {clock(src.start)}–{clock(src.end)}
                      </div>
                      {excerpt(lines, src.start, src.end).map((l) => (
                        <p key={l.id}>
                          <b>{l.channel === "mic" ? "Me" : "Them"}:</b> {l.text}
                        </p>
                      ))}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      ))}
    </div>
  );
}
