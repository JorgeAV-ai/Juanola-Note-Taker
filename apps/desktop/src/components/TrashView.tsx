import { useEffect, useState } from "react";
import { ipc, type Note } from "@/lib/ipc";
import { useLibrary } from "@/state/library";
import { Button } from "@/components/ui/button";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { NoteRow } from "@/components/NoteList";
import { useT } from "@/lib/i18n";

// Papelera: restaurar, o borrar para siempre (con confirmación: no se puede deshacer).
export function TrashView() {
  const t = useT();
  const [notes, setNotes] = useState<Note[]>([]);
  const [confirm, setConfirm] = useState<Note | null>(null);
  const reload = useLibrary((s) => s.reload);
  const refresh = () => ipc.listTrashedNotes().then(setNotes).catch(() => setNotes([]));

  useEffect(() => {
    refresh();
  }, []);

  const restore = async (id: string) => {
    await ipc.restoreNote(id);
    await Promise.all([refresh(), reload()]);
  };
  const destroy = async (id: string) => {
    setConfirm(null);
    await ipc.deleteNoteForever(id);
    await refresh();
  };

  return (
    <div className="trash mx-auto max-w-[860px] px-10 pb-16">
      <h1 className="page-title mb-8 mt-6 font-serif text-[34px] font-normal leading-tight">{t("nav.trash")}</h1>
      {notes.length === 0 ? (
        <p className="empty px-1 text-[14.5px] text-muted">{t("trash.empty")}</p>
      ) : (
        <ul className="m-0 list-none p-0">
          {notes.map((n) => (
            <li key={n.id}>
              <NoteRow
                note={n}
                right={
                  <span className="row-actions flex items-center gap-2">
                    <Button size="sm" className="restore" onClick={() => restore(n.id)}>
                      {t("trash.restore")}
                    </Button>
                    <Button size="sm" variant="ghost" className="destroy" onClick={() => setConfirm(n)}>
                      {t("trash.delete")}
                    </Button>
                  </span>
                }
              />
            </li>
          ))}
        </ul>
      )}
      <AlertDialog open={!!confirm} onOpenChange={(o) => !o && setConfirm(null)}>
        <AlertDialogContent className="note-confirm">
          <AlertDialogTitle>{t("trash.confirmTitle")}</AlertDialogTitle>
          <AlertDialogDescription>{t("trash.confirmDesc", { title: confirm?.title || t("common.untitled") })}</AlertDialogDescription>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction destructive onClick={() => confirm && destroy(confirm.id)}>
              {t("trash.deleteForever")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
