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

// Papelera: restaurar, o borrar para siempre (con confirmación: no se puede deshacer).
export function TrashView() {
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
      <h1 className="page-title mb-8 mt-6 font-serif text-[34px] font-normal leading-tight">Papelera</h1>
      {notes.length === 0 ? (
        <p className="empty px-1 text-[14.5px] text-muted">La papelera está vacía.</p>
      ) : (
        <ul className="m-0 list-none p-0">
          {notes.map((n) => (
            <li key={n.id}>
              <NoteRow
                note={n}
                right={
                  <span className="row-actions flex items-center gap-2">
                    <Button size="sm" className="restore" onClick={() => restore(n.id)}>
                      Restaurar
                    </Button>
                    <Button size="sm" variant="ghost" className="destroy" onClick={() => setConfirm(n)}>
                      Borrar…
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
          <AlertDialogTitle>¿Borrar para siempre?</AlertDialogTitle>
          <AlertDialogDescription>«{confirm?.title || "Untitled"}» se borrará con su transcripción. No se puede deshacer.</AlertDialogDescription>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancelar</AlertDialogCancel>
            <AlertDialogAction destructive onClick={() => confirm && destroy(confirm.id)}>
              Borrar para siempre
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
