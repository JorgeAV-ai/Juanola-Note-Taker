import { useEffect, useMemo, useState } from "react";
import { FileText, Search } from "lucide-react";
import { ipc, type Note } from "@/lib/ipc";
import { shortDate } from "@/lib/dates";
import { useT } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";

// Texto buscable de una nota: título, notas crudas y notas mejoradas.
function noteText(n: Note): string {
  const parts: string[] = [];
  const walk = (node: unknown) => {
    if (!node || typeof node !== "object") return;
    const o = node as { text?: string; content?: unknown[] };
    if (o.text) parts.push(o.text);
    o.content?.forEach(walk);
  };
  walk(n.raw_doc);
  n.enhanced_doc?.sections.forEach((s) => {
    parts.push(s.heading);
    s.bullets.forEach((b) => parts.push(b.text));
  });
  return parts.join(" · ");
}

function snippet(text: string, q: string): [string, string, string] | null {
  const i = text.toLowerCase().indexOf(q.toLowerCase());
  if (i < 0) return null;
  const from = Math.max(0, i - 30);
  return [(from > 0 ? "…" : "") + text.slice(from, i), text.slice(i, i + q.length), text.slice(i + q.length, i + q.length + 90)];
}

// Búsqueda (⌘K): campo arriba, «ESC», y resultados con el
// fragmento que coincide resaltado.
export function SearchDialog({ open, onOpenChange, onOpenNote }: { open: boolean; onOpenChange: (o: boolean) => void; onOpenNote: (id: string) => void }) {
  const t = useT();
  const [q, setQ] = useState("");
  const [notes, setNotes] = useState<Note[]>([]);
  const [sel, setSel] = useState(0);

  useEffect(() => {
    if (!open) return;
    setQ("");
    setSel(0);
    ipc.listNotes().then(setNotes).catch(() => setNotes([]));
  }, [open]);

  const results = useMemo(() => {
    const term = q.trim();
    if (!term) return notes.slice(0, 6).map((n) => ({ n, snip: null as [string, string, string] | null }));
    return notes
      .map((n) => {
        const title = n.title || t("common.untitled");
        const inTitle = title.toLowerCase().includes(term.toLowerCase());
        const snip = snippet(noteText(n), term);
        return inTitle || snip ? { n, snip } : null;
      })
      .filter((x): x is { n: Note; snip: [string, string, string] | null } => !!x);
  }, [q, notes]);

  const go = (id: string) => {
    onOpenChange(false);
    onOpenNote(id);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="search-dialog" aria-describedby={undefined}>
        <DialogTitle className="sr-only">{t("common.search")}</DialogTitle>
        <div className="flex items-center gap-3 border-b border-line px-7 py-5">
          <input
            autoFocus
            className="search-input min-w-0 flex-1 bg-transparent text-[18px] outline-none placeholder:text-muted"
            placeholder={t("search.placeholder")}
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              setSel(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") setSel((s) => Math.min(s + 1, results.length - 1));
              if (e.key === "ArrowUp") setSel((s) => Math.max(s - 1, 0));
              if (e.key === "Enter" && results[sel]) go(results[sel].n.id);
            }}
          />
          <kbd className="rounded-md border border-line px-1.5 py-0.5 font-sans text-[11.5px] font-medium text-ink-2">ESC</kbd>
        </div>
        <div className="max-h-[55vh] overflow-auto p-3">
          {q.trim() && (
            <div className="mb-2 flex items-center gap-3 rounded-2xl bg-hover/70 px-4 py-3.5 text-[16px]">
              <Search className="size-5 text-ink-2" />
              <span className="flex-1">
                {t("search.searchFor")} <b className="font-semibold">{q.trim()}</b>
              </span>
              <span className="text-[14px] text-muted">
                {results.length} {results.length === 1 ? t("search.result1") : t("search.resultN")}
              </span>
            </div>
          )}
          <div className="px-4 pb-1 pt-2 text-[14px] text-muted">{q.trim() ? t("search.more") : t("search.recent")}</div>
          {results.length === 0 && <p className="px-4 py-3 text-[14.5px] text-muted">{t("search.none", { q: q.trim() })}</p>}
          {results.map(({ n, snip }, i) => (
            <button
              key={n.id}
              className={cn("search-result flex w-full items-start gap-4 rounded-2xl px-4 py-3 text-left", i === sel ? "bg-hover/70" : "hover:bg-hover/50")}
              data-note-id={n.id}
              onMouseEnter={() => setSel(i)}
              onClick={() => go(n.id)}
            >
              <FileText className="mt-0.5 size-5 shrink-0 text-ink-2" strokeWidth={1.5} />
              <span className="min-w-0 flex-1">
                <span className="flex items-baseline gap-3">
                  <span className="flex-1 truncate text-[16px]">{n.title || t("common.untitled")}</span>
                  <span className="shrink-0 text-[14px] text-muted">{shortDate(new Date(n.created_at))}</span>
                </span>
                {snip && (
                  <span className="mt-0.5 block truncate text-[14.5px] text-muted">
                    {snip[0]}
                    <mark className="rounded-sm bg-mark text-ink-2">{snip[1]}</mark>
                    {snip[2]}
                  </span>
                )}
              </span>
            </button>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
