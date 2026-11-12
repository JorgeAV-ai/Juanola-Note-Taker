import { useEffect, useRef, useState } from "react";
import { useEditor, EditorContent, type Editor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Placeholder from "@tiptap/extension-placeholder";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";
import Image from "@tiptap/extension-image";
import Underline from "@tiptap/extension-underline";
import { LayoutTemplate } from "lucide-react";
import { FormatMenu } from "@/components/FormatMenu";
import type { Template } from "@/lib/ipc";
import { t } from "@/lib/i18n";
import { cn } from "@/lib/utils";

// Una nota nueva llega como `{type: "doc", content: []}`: sin ningún bloque,
// ProseMirror no tiene párrafo donde poner «Write notes» y pinta en su lugar
// el gapcursor (una raya sobre el cursor). Vacío = empezar con un párrafo.
function emptyDoc(doc: unknown): boolean {
  if (doc == null || doc === "") return true;
  const content = (doc as { content?: unknown[] }).content;
  return Array.isArray(content) && content.length === 0;
}

const fold = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .trim();

/** Menú `/`: el párrafo `[before, after)` contiene solo «/consulta» y el cursor está al final. */
type Slash = { query: string; before: number; after: number; left: number; top: number };

function detectSlash(editor: Editor): Slash | null {
  const { selection } = editor.state;
  const { $from } = selection;
  if (!selection.empty || $from.parent.type.name !== "paragraph" || $from.parentOffset !== $from.parent.content.size) return null;
  const m = /^\/([^\s/]{0,40})$/.exec($from.parent.textContent);
  if (!m) return null;
  const coords = editor.view.coordsAtPos($from.start());
  return { query: m[1], before: $from.before(), after: $from.after(), left: coords.left, top: coords.bottom + 6 };
}

// Editor de notas crudas. Markdown ligero (#, -, [], **, *) lo da StarterKit + TaskList.
// Guarda el documento ProseMirror (debounce de 600 ms) con `onChange`.
// Con `templates`, «/» en una línea vacía abre el menú de plantillas: la
// elegida pone sus apartados como títulos y se avisa con `onPickTemplate`.
export function NoteEditor({
  initial,
  onChange,
  templates = [],
  onPickTemplate,
}: {
  initial: unknown;
  onChange: (doc: unknown) => void;
  templates?: Template[];
  onPickTemplate?: (tpl: Template) => void;
}) {
  const timer = useRef<number | undefined>(undefined);
  const [slash, setSlash] = useState<Slash | null>(null);
  const [active, setActive] = useState(0);
  // Párrafo donde se cerró el menú con Escape: no se vuelve a abrir ahí
  // hasta que deje de ser «/…».
  const dismissed = useRef<number | null>(null);

  const items = slash ? templates.filter((x) => fold(x.name).includes(fold(slash.query))) : [];
  // El editor se crea una vez: sus manejadores leen el estado actual por aquí.
  const live = useRef({ slash, items, active, pick: (_: Template) => {} });

  const update = (editor: Editor) => {
    if (!onPickTemplate) return;
    const s = detectSlash(editor);
    if (!s) dismissed.current = null;
    const open = s && dismissed.current !== s.before ? s : null;
    if (open && live.current.slash?.query !== open.query) setActive(0);
    setSlash(open);
  };

  const editor = useEditor({
    extensions: [
      StarterKit,
      Underline,
      Placeholder.configure({ placeholder: t("note.writeNotes") }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Image.configure({ inline: false, allowBase64: true }),
    ],
    content: emptyDoc(initial) ? "" : (initial as object),
    onUpdate: ({ editor }) => {
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => onChange(editor.getJSON()), 600);
    },
    onTransaction: ({ editor }) => update(editor),
    onBlur: () => setSlash(null),
    editorProps: {
      handleKeyDown: (_view, event) => {
        const { slash, items, active, pick } = live.current;
        if (!slash) return false;
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          if (items.length) setActive((active + (event.key === "ArrowDown" ? 1 : items.length - 1)) % items.length);
          return true;
        }
        if ((event.key === "Enter" || event.key === "Tab") && items[active]) {
          pick(items[active]);
          return true;
        }
        if (event.key === "Escape") {
          dismissed.current = slash.before;
          setSlash(null);
          return true;
        }
        return false;
      },
    },
  });

  const pick = (tpl: Template) => {
    if (!editor || !slash) return;
    // El párrafo «/…» se sustituye por un título por apartado, cada uno con un
    // párrafo vacío debajo, y el cursor queda bajo el primero.
    const content = tpl.structure.flatMap((s) => [
      { type: "heading", attrs: { level: 3 }, content: [{ type: "text", text: s.heading }] },
      { type: "paragraph" },
    ]);
    const firstHeading = tpl.structure[0]?.heading.length ?? 0;
    editor
      .chain()
      .focus()
      .insertContentAt({ from: slash.before, to: slash.after }, content)
      .setTextSelection(slash.before + firstHeading + 2 + 1)
      .run();
    setSlash(null);
    onPickTemplate?.(tpl);
  };
  live.current = { slash, items, active, pick };

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
      {slash && (
        <div
          className="slash-menu fixed z-50 w-[280px] overflow-hidden rounded-2xl border border-line bg-popover p-1.5 text-popover-foreground shadow-[0_12px_32px_rgba(0,0,0,0.10)]"
          style={{ left: slash.left, top: slash.top }}
          role="listbox"
          aria-label={t("note.slashTemplates")}
          // Sin esto, el clic quita el foco al editor (onBlur cierra el menú) antes de elegir.
          onMouseDown={(e) => e.preventDefault()}
        >
          <div className="px-2.5 py-1.5 text-[12px] font-medium text-muted">{t("note.slashTemplates")}</div>
          {items.length === 0 && <div className="px-2.5 py-1.5 text-[13.5px] text-muted">{t("note.slashNone")}</div>}
          {items.map((tpl, i) => (
            <div
              key={tpl.id}
              role="option"
              aria-selected={i === active}
              data-template-id={tpl.id}
              className={cn("flex cursor-default items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-[14px]", i === active && "bg-hover")}
              onMouseEnter={() => setActive(i)}
              onClick={() => pick(tpl)}
            >
              <LayoutTemplate className="size-4 shrink-0 text-ink-2" />
              <span className="min-w-0 flex-1">
                <span className="block truncate">{tpl.name}</span>
                <span className="block truncate text-[12px] text-muted">{tpl.structure.map((s) => s.heading).join(" · ")}</span>
              </span>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
