import { useEffect, useRef } from "react";
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Placeholder from "@tiptap/extension-placeholder";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";
import Image from "@tiptap/extension-image";
import Underline from "@tiptap/extension-underline";
import { FormatMenu } from "@/components/FormatMenu";

// Una nota nueva llega como `{type: "doc", content: []}`: sin ningún bloque,
// ProseMirror no tiene párrafo donde poner «Write notes» y pinta en su lugar
// el gapcursor (una raya sobre el cursor). Vacío = empezar con un párrafo.
function emptyDoc(doc: unknown): boolean {
  if (doc == null || doc === "") return true;
  const content = (doc as { content?: unknown[] }).content;
  return Array.isArray(content) && content.length === 0;
}

// Editor de notas crudas. Markdown ligero (#, -, [], **, *) lo da StarterKit + TaskList.
// Guarda el documento ProseMirror (debounce de 600 ms) con `onChange`.
export function NoteEditor({ initial, onChange }: { initial: unknown; onChange: (doc: unknown) => void }) {
  const timer = useRef<number | undefined>(undefined);
  const editor = useEditor({
    extensions: [
      StarterKit,
      Underline,
      Placeholder.configure({ placeholder: "Write notes" }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Image.configure({ inline: false, allowBase64: true }),
    ],
    content: emptyDoc(initial) ? "" : (initial as object),
    onUpdate: ({ editor }) => {
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => onChange(editor.getJSON()), 600);
    },
  });
  // Guardar lo pendiente al salir de la nota.
  useEffect(
    () => () => {
      if (timer.current !== undefined && editor) {
        window.clearTimeout(timer.current);
        onChange(editor.getJSON());
      }
    },
    [editor],
  );
  return (
    <>
      <FormatMenu editor={editor} />
      <EditorContent editor={editor} className="editor" />
    </>
  );
}
