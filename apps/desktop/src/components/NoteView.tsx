import { useEffect, useRef, useState } from "react";
import { AlignLeft, CalendarDays, ChevronDown, Ellipsis, Folder, FolderMinus, FolderPlus, Sparkles, Trash2, UsersRound } from "lucide-react";
import { events, ipc, type CalendarEvent, type Note, MODELS_MISSING } from "@/lib/ipc";
import { useModels } from "@/state/models";
import { dayLabel, timeLabel } from "@/lib/dates";
import { cn } from "@/lib/utils";
import { t as tr, tBackend, useT } from "@/lib/i18n";
import { useTranscript } from "@/state/transcript";
import { useLibrary } from "@/state/library";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { EnhancedView } from "@/components/EnhancedView";
import { NoteEditor } from "@/components/NoteEditor";
import { RecordingPill } from "@/components/RecordingPill";
import { AskAnythingBar } from "@/components/AskAnythingBar";

type Tab = "mine" | "enhanced";

// Vista de nota: título en serif, fila de chips (My notes / Enhanced,
// fecha y asistentes, carpeta, más) y abajo la píldora de grabación junto a
// «Ask anything». Al parar la grabación se generan las notas mejoradas; si el
// usuario ya las había editado, se pregunta antes de sustituirlas.
export function NoteView({ id, autostart, onTrashed }: { id: string; autostart?: number; onTrashed: () => void }) {
  const t = useT();
  const [note, setNote] = useState<Note | null>(null);
  const [tab, setTab] = useState<Tab>("mine");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [folder, setFolder] = useState<string | null>(null);
  const [event, setEvent] = useState<CalendarEvent | null>(null);
  const folders = useLibrary((s) => s.folders);
  const reloadFolders = useLibrary((s) => s.reload);
  const load = useTranscript((s) => s.load);
  const titleTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    setNote(null);
    ipc.getNote(id).then((n) => {
      setNote(n);
      if (n?.enhanced_doc) setTab("enhanced");
    });
    load(id);
    ipc.getNoteFolder(id).then(setFolder).catch(() => setFolder(null));
    ipc.noteEvent(id).then(setEvent).catch(() => setEvent(null));
    const un = events.noteStatus((e) => {
      if (e.note_id !== id) return;
      setBusy(e.status === "enhancing");
      if (e.status === "failed") setError(e.message ?? tr("note.enhanceFailed"));
    });
    return () => {
      un.then((f) => f());
    };
  }, [id, load]);

  const enhanceNow = async () => {
    setConfirm(false);
    setError(null);
    setBusy(true);
    try {
      const enhanced = await ipc.enhanceNote(id);
      setNote((n) => (n ? { ...n, enhanced_doc: enhanced, status: "ready" } : n));
      setTab("enhanced");
    } catch (e) {
      if (String(e) === MODELS_MISSING) useModels.getState().openDialog(true);
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const requestEnhance = () => (note?.enhanced_doc?.edited ? setConfirm(true) : enhanceNow());

  const moveTo = async (folderId: string | null) => {
    try {
      await ipc.setNoteFolder(id, folderId);
      setFolder(folderId);
      await reloadFolders();
    } catch (e) {
      setError(String(e));
    }
  };
  const trash = async () => {
    try {
      await ipc.trashNote(id);
      await reloadFolders();
      onTrashed();
    } catch (e) {
      setError(String(e));
    }
  };

  if (!note) return <div className="note" />;
  const folderName = folders.find((f) => f.id === folder)?.name;
  const hasEnhanced = !!note.enhanced_doc;

  return (
    <div className="note relative mx-auto flex min-h-full max-w-[800px] flex-col px-12" data-note-id={id}>
      <input
        className="note-title mt-4 w-full bg-transparent font-serif text-[38px] font-normal leading-tight outline-none placeholder:text-faint"
        placeholder={t("note.newNote")}
        defaultValue={note.title}
        onChange={(e) => {
          const t = e.target.value;
          window.clearTimeout(titleTimer.current);
          titleTimer.current = window.setTimeout(() => ipc.setNoteTitle(id, t).catch((e) => setError(String(e))), 400);
        }}
      />

      <div className="note-tabs mb-7 mt-4 flex flex-wrap items-center gap-2.5">
        {hasEnhanced ? (
          <div className="flex h-10 items-center rounded-full border border-line bg-hover/60 p-[3px]">
            <button
              className={cn("tab flex h-full items-center rounded-full px-3 text-ink-2 transition-colors", tab === "mine" && "active bg-surface text-ink shadow-sm")}
              onClick={() => setTab("mine")}
              title={t("nav.myNotes")}
              aria-label={t("nav.myNotes")}
            >
              <AlignLeft className="size-[18px]" />
              <span className="sr-only">{t("nav.myNotes")}</span>
            </button>
            <div className={cn("flex h-full items-center rounded-full transition-colors", tab === "enhanced" && "bg-surface shadow-sm")}>
              <button className={cn("tab flex h-full items-center gap-2 pl-3.5 pr-1 text-[15px]", tab === "enhanced" ? "active text-ink" : "text-ink-2")} onClick={() => setTab("enhanced")}>
                <Sparkles className="size-[17px]" />
                {t("note.enhanced")}
              </button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button className="enhance-menu flex h-full items-center pl-0.5 pr-3 text-ink-2" aria-label={t("note.enhanceOptions")} disabled={busy}>
                    <ChevronDown className="size-4" />
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start">
                  <DropdownMenuItem title={t("note.generate")} onSelect={requestEnhance}>
                    <Sparkles /> {t("note.regenerate")}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        ) : (
          <Button size="lg" title={t("note.generate")} onClick={requestEnhance} disabled={busy} className="text-[15px]">
            <Sparkles /> {t("note.enhance")}
          </Button>
        )}

        <div className="note-meta flex h-10 items-center gap-3 rounded-full border border-line px-4 text-[15px] text-ink-2">
          <span className="flex items-center gap-2">
            <CalendarDays className="size-[17px]" /> {dayLabel(new Date(event?.start ?? note.created_at))}
            {event && <span className="text-muted">{timeLabel(new Date(event.start))}</span>}
          </span>
          <span className="flex items-center gap-2" title={event ? attendeeNames(event) : undefined}>
            <UsersRound className="size-[17px]" /> {event ? attendeeLabel(event) : t("common.me")}
          </span>
        </div>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button size="lg" className="note-folder text-[15px] font-normal text-ink-2" data-folder={folder ?? ""}>
              {folderName ? <Folder /> : <FolderPlus />}
              {folderName ?? t("note.addToFolder")}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-[230px]">
            <DropdownMenuLabel>{t("note.moveTo")}</DropdownMenuLabel>
            {folders.length === 0 && <div className="px-2.5 py-1.5 text-[13.5px] text-muted">{t("note.noFolders")}</div>}
            {folders.map((f) => (
              <DropdownMenuItem key={f.id} data-folder-id={f.id} onSelect={() => moveTo(f.id)}>
                <Folder /> <span className="flex-1 truncate">{f.name}</span>
                {f.id === folder && <span className="text-[12px] text-muted">{t("note.current")}</span>}
              </DropdownMenuItem>
            ))}
            {folder && (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuItem onSelect={() => moveTo(null)}>
                  <FolderMinus /> {t("note.removeFromFolder")}
                </DropdownMenuItem>
              </>
            )}
          </DropdownMenuContent>
        </DropdownMenu>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button size="icon" className="note-more size-10" aria-label={t("note.more")}>
              <Ellipsis />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            <DropdownMenuItem destructive title={t("note.moveToTrash")} onSelect={trash}>
              <Trash2 /> {t("note.moveToTrash")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

      </div>

      {error && <div className="note-error mb-4 rounded-xl border border-[#f1d3a8] bg-[#fdf6ea] px-4 py-2.5 text-[13.5px] text-[#8a4b0f] dark:border-[#6b4a1e] dark:bg-[#3a2e1c] dark:text-[#f3c98b]">{tBackend(error)}</div>}

      <div className="flex-1">
        {tab === "mine" || !note.enhanced_doc ? (
          <NoteEditor key={`${id}-${t("note.writeNotes")}`} initial={note.raw_doc} onChange={(doc) => ipc.setNoteRawDoc(id, doc).catch((e) => setError(String(e)))} />
        ) : (
          <EnhancedView
            notes={note.enhanced_doc}
            onChange={(doc) => {
              setNote((n) => (n ? { ...n, enhanced_doc: doc } : n));
              ipc.setNoteEnhancedDoc(id, doc).catch((e) => setError(String(e)));
            }}
          />
        )}
      </div>

      <div className="sticky bottom-0 -mx-12 mt-6 bg-gradient-to-t from-canvas from-70% to-transparent px-12 pb-6 pt-10">
        {busy && (
          <div className="enhancing mb-4 flex h-[52px] items-center gap-3 rounded-full bg-hover px-5 text-[15px] font-medium text-ink">
            <span className="licorice" aria-hidden="true"><i /></span>
            {t("note.enhancing")}
          </div>
        )}
        <RecordingPill
          noteId={id}
          autostart={autostart}
          onStopped={(saved) => (saved > 0 ? requestEnhance() : setError(t("note.nothingTranscribed")))}
          askBar={<AskAnythingBar recipe={t("note.followUp")} onAsk={() => setError(t("note.chatNotConnected"))} />}
        />
      </div>

      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent className="note-confirm">
          <AlertDialogTitle>{t("note.regenTitle")}</AlertDialogTitle>
          <AlertDialogDescription>{t("note.regenDesc")}</AlertDialogDescription>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("note.keep")}</AlertDialogCancel>
            <AlertDialogAction onClick={enhanceNow}>{t("note.regenerate")}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

const personName = (a: CalendarEvent["attendees"][number]) => (a.is_self ? tr("common.me") : a.name || a.email.split("@")[0]);
const attendeeNames = (e: CalendarEvent) => e.attendees.filter((a) => !a.declined).map(personName).join(", ");

// «Me», «Ana», «Ana +3»: los extra se colapsan en «+N».
function attendeeLabel(e: CalendarEvent) {
  const others = e.attendees.filter((a) => !a.is_self && !a.declined);
  if (others.length === 0) return tr("common.me");
  return others.length === 1 ? personName(others[0]) : `${personName(others[0])} +${others.length - 1}`;
}
