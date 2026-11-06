import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Placeholder from "@tiptap/extension-placeholder";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";
import Image from "@tiptap/extension-image";

// Editor de notas crudas. Markdown ligero (#, -, [], **, *) lo da StarterKit + TaskList.
// Fase 1: marca `ai-text` (gris) y nodo `transcript-ref` (lupa) para las notas mejoradas.
export function NoteEditor() {
  const editor = useEditor({
    extensions: [
      StarterKit,
      Placeholder.configure({ placeholder: "Write notes..." }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Image.configure({ inline: false, allowBase64: true }),
    ],
    content: "",
  });
  return <EditorContent editor={editor} className="editor" />;
}
