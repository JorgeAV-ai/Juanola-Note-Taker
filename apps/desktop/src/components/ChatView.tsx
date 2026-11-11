import { useEffect, useState } from "react";
import { ArrowUp, ChevronDown, FileText, Paperclip } from "lucide-react";
import { ipc, type Note } from "@/lib/ipc";
import { useT } from "@/lib/i18n";

const ago = (d: Date) => {
  const m = Math.round((Date.now() - d.getTime()) / 60000);
  if (m < 60) return `${Math.max(m, 1)}m`;
  if (m < 60 * 24) return `${Math.round(m / 60)}h`;
  return `${Math.round(m / 1440)}d`;
};

// Chat de Granola: saludo en serif, compositor grande y recientes. El modelo aún
// no está conectado al chat, así que enviar solo lo explica.
export function ChatView({ initial, onOpenNote }: { initial?: string; onOpenNote: (id: string) => void }) {
  const t = useT();
  const [q, setQ] = useState(initial ?? "");
  const [sent, setSent] = useState(false);
  const [notes, setNotes] = useState<Note[]>([]);
  useEffect(() => {
    ipc.listNotes().then((n) => setNotes(n.slice(0, 5))).catch(() => setNotes([]));
  }, []);

  return (
    <div className="chat mx-auto max-w-[820px] px-10 pb-16">
      <h1 className="page-title mb-9 mt-14 font-serif text-[42px] font-normal leading-tight">{t("chat.hello")}</h1>
      <form
        className="rounded-[28px] border border-line bg-surface p-5 shadow-[0_0_0_5px_rgba(86,105,31,0.08)]"
        onSubmit={(e) => {
          e.preventDefault();
          if (q.trim()) setSent(true);
        }}
      >
        <textarea
          className="chat-input block min-h-[56px] w-full resize-none bg-transparent px-2 text-[17px] leading-relaxed outline-none placeholder:text-muted"
          placeholder={t("chat.placeholder")}
          value={q}
          rows={2}
          onChange={(e) => {
            setQ(e.target.value);
            setSent(false);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              if (q.trim()) setSent(true);
            }
          }}
        />
        <div className="mt-2 flex items-center gap-3 px-1">
          <Paperclip className="size-5 text-muted" />
          <span className="flex items-center gap-1 text-[15px] text-muted">
            Auto <ChevronDown className="size-4" />
          </span>
          <span className="flex-1" />
          <button type="submit" disabled={!q.trim()} className="flex size-11 items-center justify-center rounded-full bg-olive text-canvas transition-opacity disabled:opacity-40" aria-label={t("chat.send")}>
            <ArrowUp className="size-5" />
          </button>
        </div>
      </form>
      {sent && <p className="mt-4 px-2 text-[14px] text-ink-2">{t("chat.notConnected")}</p>}

      <h2 className="mb-3 mt-12 px-1 text-[14.5px] text-muted">{t("chat.recents")}</h2>
      <ul className="m-0 list-none p-0">
        {notes.map((n) => (
          <li key={n.id}>
            <button className="flex w-full items-center gap-4 rounded-xl px-1 py-2 text-left hover:bg-hover" onClick={() => onOpenNote(n.id)}>
              <span className="flex size-11 shrink-0 items-center justify-center rounded-xl border border-line bg-surface text-ink-2">
                <FileText className="size-5" strokeWidth={1.5} />
              </span>
              <span className="flex-1 truncate text-[16px]">{n.title || t("common.untitled")}</span>
              <span className="text-[14px] text-muted">{ago(new Date(n.created_at))}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
