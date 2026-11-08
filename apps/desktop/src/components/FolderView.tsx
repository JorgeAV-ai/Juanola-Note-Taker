import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
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
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { NoteList } from "@/components/NoteList";

// Carpeta: nombre editable en serif, sus notas por día y borrar la carpeta
// (las notas no se borran: quedan sin carpeta). «+ New note» de la barra
// superior crea la nota dentro.
export function FolderView({ id, onOpenNote, onNewNote, onDeleted }: { id: string; onOpenNote: (id: string) => void; onNewNote: () => void; onDeleted: () => void }) {
  const folder = useLibrary((s) => s.folders.find((f) => f.id === id));
  const reload = useLibrary((s) => s.reload);
  const [notes, setNotes] = useState<Note[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    ipc.listFolderNotes(id).then(setNotes).catch((e) => setError(String(e)));
  }, [id]);

  if (!folder) return <div />;

  const rename = async (name: string) => {
    if (!name.trim() || name.trim() === folder.name) return;
    try {
      await ipc.renameFolder(id, name);
      await reload();
    } catch (e) {
      setError(String(e));
    }
  };
  const remove = async () => {
    await ipc.deleteFolder(id);
    await reload();
    onDeleted();
  };

  return (
    <div className="folder-view mx-auto max-w-[860px] px-10 pb-16">
      <div className="mb-6 mt-6 flex items-center gap-3">
        <input
          className="note-title min-w-0 flex-1 bg-transparent font-serif text-[34px] font-normal leading-tight outline-none"
          key={folder.name}
          defaultValue={folder.name}
          aria-label="Nombre de la carpeta"
          onBlur={(e) => rename(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        />
        <AlertDialog>
          <AlertDialogTrigger asChild>
            <Button variant="ghost" className="delete-folder text-ink-2">
              <Trash2 /> Borrar carpeta
            </Button>
          </AlertDialogTrigger>
          <AlertDialogContent className="note-confirm">
            <AlertDialogTitle>¿Borrar «{folder.name}»?</AlertDialogTitle>
            <AlertDialogDescription>Sus notas no se borran: quedan en Home, sin carpeta.</AlertDialogDescription>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancelar</AlertDialogCancel>
              <AlertDialogAction destructive onClick={remove}>
                Borrar carpeta
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </div>
      <p className="mb-6 px-1 text-[14px] text-muted">{notes.length === 1 ? "1 nota" : `${notes.length} notas`}</p>
      {error && <div className="note-error mb-4 text-[13.5px] text-danger">{error}</div>}
      <NoteList notes={notes} onOpen={onOpenNote} empty="Carpeta vacía. Crea una nota con «+ New note» o muévela desde la propia nota." />
      {notes.length === 0 && (
        <Button className="mt-4" onClick={onNewNote}>
          Nueva nota aquí
        </Button>
      )}
    </div>
  );
}
