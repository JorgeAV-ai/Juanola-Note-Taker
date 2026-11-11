import { FileText } from "lucide-react";
import type { Note } from "@/lib/ipc";
import { groupByDay, timeLabel } from "@/lib/dates";
import { useT } from "@/lib/i18n";

// Notas agrupadas por día: icono en un cuadrado,
// título, «Me» debajo y la hora a la derecha.
export function NoteList({ notes, onOpen, empty }: { notes: Note[]; onOpen: (id: string) => void; empty: string }) {
  useT();
  if (notes.length === 0) return <p className="empty px-1 text-[14px] text-muted">{empty}</p>;
  return (
    <div className="flex flex-col gap-6">
      {groupByDay(notes, (n) => new Date(n.created_at)).map(([label, group]) => (
        <section key={label}>
          <h3 className="mb-1.5 px-1 text-[13.5px] font-medium text-muted">{label}</h3>
          <ul className="rows m-0 list-none p-0">
            {group.map((n) => (
              <li key={n.id}>
                <NoteRow note={n} onOpen={onOpen} />
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}

export function NoteRow({ note, onOpen, right }: { note: Note; onOpen?: (id: string) => void; right?: React.ReactNode }) {
  const t = useT();
  return (
    <div
      className="row group flex cursor-pointer items-center gap-3.5 rounded-xl px-2 py-2 transition-colors hover:bg-hover"
      data-note-id={note.id}
      onClick={() => onOpen?.(note.id)}
    >
      <span className="flex size-9 shrink-0 items-center justify-center rounded-lg border border-line bg-card text-ink-2">
        <FileText className="size-[17px]" />
      </span>
      <span className="min-w-0 flex-1">
        <span className="row-title block truncate text-[15px] text-ink">{note.title || t("common.untitled")}</span>
        <span className="block text-[13px] text-muted">{t("common.me")}</span>
      </span>
      {right ?? <span className="row-meta text-[13px] text-muted">{timeLabel(new Date(note.created_at))}</span>}
    </div>
  );
}
