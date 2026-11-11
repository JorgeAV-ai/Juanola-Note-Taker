import { BubbleMenu, type Editor } from "@tiptap/react";
import { Bold, Italic, Strikethrough, Underline } from "lucide-react";
import { useT } from "@/lib/i18n";

// Menú flotante al seleccionar texto (negrita, cursiva, subrayado, tachado).
// El BubbleMenu de TipTap mueve su elemento fuera del árbol de React (a tippy);
// va dentro de un contenedor propio para que, al desmontar, React quite el
// contenedor y no ese elemento (si no: «removeChild… not a child» y la app
// entera se desmonta).
export function FormatMenu({ editor }: { editor: Editor | null }) {
  const t = useT();
  if (!editor) return null;
  const items = [
    { name: "bold", icon: <Bold className="size-[17px]" />, run: () => editor.chain().focus().toggleBold().run(), label: t("note.bold") },
    { name: "italic", icon: <Italic className="size-[17px]" />, run: () => editor.chain().focus().toggleItalic().run(), label: t("note.italic") },
    { name: "underline", icon: <Underline className="size-[17px]" />, run: () => editor.chain().focus().toggleUnderline().run(), label: t("note.underline") },
    { name: "strike", icon: <Strikethrough className="size-[17px]" />, run: () => editor.chain().focus().toggleStrike().run(), label: t("note.strike") },
  ];
  return (
    <div className="contents">
    <BubbleMenu editor={editor} tippyOptions={{ duration: 100, placement: "top-start" }} className="bubble-menu">
      {items.map((it) => (
        <button key={it.name} type="button" title={it.label} aria-label={it.label} className={editor.isActive(it.name) ? "on" : ""} onClick={it.run}>
          {it.icon}
        </button>
      ))}
    </BubbleMenu>
    </div>
  );
}
