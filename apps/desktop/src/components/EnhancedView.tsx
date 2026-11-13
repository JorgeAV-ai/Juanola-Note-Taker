import { useEffect, useRef, useState } from "react";
import { Mark, Node, NodeViewWrapper, ReactNodeViewRenderer, Extension, EditorContent, useEditor, type NodeViewProps } from "@tiptap/react";
import { Plugin } from "@tiptap/pm/state";
import StarterKit from "@tiptap/starter-kit";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";
import { ZoomIn } from "lucide-react";
import type { Citation, EnhancedNotes } from "@/lib/ipc";
import { fromDoc, toDoc } from "@/lib/enhancedDoc";
import { excerpt, useTranscript } from "@/state/transcript";
import { FormatMenu } from "@/components/FormatMenu";
import { useT } from "@/lib/i18n";

const clock = (s: number) => `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

// Texto de la IA (gris). No es inclusiva: lo que se escribe al borde no la hereda.
const AiMark = Mark.create({
  name: "ai",
  inclusive: false,
  parseHTML: () => [{ tag: "span.ai-text" }],
  renderHTML: () => ["span", { class: "ai-text" }, 0],
});

// Lo que el usuario escribe o pega, aunque sea en mitad de una frase de la IA,
// pasa a ser texto suyo (negro).
const UserEdits = Extension.create({
  name: "userEdits",
  addProseMirrorPlugins() {
    const ai = this.editor.schema.marks.ai;
    return [
      new Plugin({
        appendTransaction: (trs, _old, state) => {
          if (!trs.some((t) => t.docChanged && !t.getMeta("fromLoad"))) return null;
          const ranges: [number, number][] = [];
          for (const t of trs) {
            if (!t.docChanged || t.getMeta("fromLoad")) continue;
            t.mapping.maps.forEach((map, i) => {
              const rest = t.mapping.slice(i + 1);
              map.forEach((_a, _b, from, to) => {
                if (to > from) ranges.push([rest.map(from, -1), rest.map(to, 1)]);
              });
            });
          }
          if (!ranges.length) return null;
          const tr = state.tr;
          for (const [from, to] of ranges) tr.removeMark(Math.max(0, from), Math.min(state.doc.content.size, to), ai);
          return tr.docChanged ? tr : null;
        },
      }),
    ];
  },
});

// Cita al final de la viñeta: lupa que abre el fragmento de la transcripción.
function CitationView({ node }: NodeViewProps) {
  const t = useT();
  const src = node.attrs.source as Citation | null;
  const lines = useTranscript((s) => s.lines);
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLSpanElement>(null);
  // Se cierra al pulsar fuera o con Escape.
  useEffect(() => {
    if (!open) return;
    const down = (e: MouseEvent) => {
      if (!wrap.current?.contains(e.target as globalThis.Node)) setOpen(false);
    };
    const key = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", down);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", down);
      document.removeEventListener("keydown", key);
    };
  }, [open]);
  if (!src || src.kind !== "transcript") return <NodeViewWrapper as="span" />;
  return (
    <NodeViewWrapper as="span" className="cite-wrap" contentEditable={false} ref={wrap}>
      <button className="cite" title={t("note.viewInTranscript")} aria-label={t("note.viewInTranscript")} onMouseDown={(e) => e.preventDefault()} onClick={() => setOpen((o) => !o)}>
        <ZoomIn className="size-[17px]" />
      </button>
      {open && (
        <span className="cite-pop">
          <span className="cite-label">
            {t("note.transcript")} · {clock(src.start)}–{clock(src.end)}
          </span>
          {excerpt(lines, src.start, src.end).map((l) => (
            <span className="cite-line" key={l.id}>
              <b>{l.channel === "mic" ? t("common.me") : t("common.them")}:</b> {l.text}
            </span>
          ))}
        </span>
      )}
    </NodeViewWrapper>
  );
}

const CitationNode = Node.create({
  name: "citation",
  group: "inline",
  inline: true,
  atom: true,
  selectable: false,
  addAttributes: () => ({
    source: {
      default: null,
      parseHTML: (el) => {
        try {
          return JSON.parse(el.getAttribute("data-source") ?? "null");
        } catch {
          return null;
        }
      },
      renderHTML: (attrs) => ({ "data-source": JSON.stringify(attrs.source) }),
    },
  }),
  parseHTML: () => [{ tag: "span[data-cite]" }],
  renderHTML: ({ HTMLAttributes }) => ["span", { ...HTMLAttributes, "data-cite": "" }],
  addNodeView: () => ReactNodeViewRenderer(CitationView),
});

// Notas mejoradas editables: texto del usuario en negro, de la IA en gris, y una
// lupa por viñeta con el fragmento citado. Guarda (debounce de 600 ms) como
// `EnhancedNotes` con `onChange`.
export function EnhancedView({ notes, onChange }: { notes: EnhancedNotes; onChange: (notes: EnhancedNotes) => void }) {
  const timer = useRef<number | undefined>(undefined);
  const save = useRef(onChange);
  save.current = onChange;
  // Lo último mostrado o guardado: si el padre lo devuelve como `notes`, no se
  // recarga el documento (perdería el cursor).
  const shown = useRef(notes);
  const flush = (json: Parameters<typeof fromDoc>[0]) => {
    // Todo lo que llega aquí es una edición del usuario (las cargas no guardan).
    const doc = { ...fromDoc(json), edited: true };
    shown.current = doc;
    save.current(doc);
  };
  const editor = useEditor({
    // El menú de formato lee `isActive` al renderizar (como en TipTap 2).
    shouldRerenderOnTransaction: true,
    extensions: [StarterKit.configure({ link: false, trailingNode: false }), TaskList, TaskItem.configure({ nested: true }), AiMark, CitationNode, UserEdits],
    content: toDoc(notes),
    onUpdate: ({ editor }) => {
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => {
        timer.current = undefined;
        flush(editor.getJSON());
      }, 600);
    },
  });
  // Un enhance nuevo sustituye el documento (sin contar como edición del usuario).
  useEffect(() => {
    if (!editor || shown.current === notes) return;
    shown.current = notes;
    editor.chain().setMeta("fromLoad", true).setContent(toDoc(notes), { emitUpdate: false }).run();
  }, [editor, notes]);
  useEffect(
    () => () => {
      if (timer.current !== undefined && editor) {
        window.clearTimeout(timer.current);
        flush(editor.getJSON());
      }
    },
    [editor],
  );
  return (
    <>
      <FormatMenu editor={editor} />
      <EditorContent editor={editor} className="editor enhanced" />
    </>
  );
}
