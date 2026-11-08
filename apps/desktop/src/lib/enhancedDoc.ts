// Ida y vuelta entre `EnhancedNotes` (lo que guarda el backend) y el documento
// TipTap/ProseMirror del editor de notas mejoradas.
//
// - Sección → heading (nivel 3) seguido de sus viñetas.
// - Viñeta → listItem en un bulletList; si es tarea, taskItem en un taskList.
// - Texto de la IA → marca `ai` (gris). Lo que el usuario escribe no la lleva.
// - Cita → nodo inline `citation` al final de la viñeta, con la `Citation` entera.
import type { JSONContent } from "@tiptap/react";
import type { Bullet, Citation, EnhancedNotes } from "./ipc";

export function toDoc(notes: EnhancedNotes): JSONContent {
  const content: JSONContent[] = [];
  for (const s of notes.sections) {
    content.push({ type: "heading", attrs: { level: 3 }, content: s.heading ? [{ type: "text", text: s.heading }] : [] });
    // Viñetas consecutivas del mismo tipo van en la misma lista.
    let i = 0;
    while (i < s.bullets.length) {
      const task = s.bullets[i].is_action_item;
      const run: Bullet[] = [];
      while (i < s.bullets.length && s.bullets[i].is_action_item === task) run.push(s.bullets[i++]);
      content.push({
        type: task ? "taskList" : "bulletList",
        content: run.map((b) => ({
          type: task ? "taskItem" : "listItem",
          ...(task ? { attrs: { checked: !!b.done } } : {}),
          content: [{ type: "paragraph", content: inline(b) }],
        })),
      });
    }
  }
  return { type: "doc", content };
}

function inline(b: Bullet): JSONContent[] {
  const out: JSONContent[] = [];
  if (b.text) out.push({ type: "text", text: b.text, ...(b.is_user_text ? {} : { marks: [{ type: "ai" }] }) });
  if (b.source) out.push({ type: "citation", attrs: { source: b.source } });
  return out;
}

export function fromDoc(doc: JSONContent): EnhancedNotes {
  const sections: EnhancedNotes["sections"] = [];
  const current = () => {
    if (!sections.length) sections.push({ heading: "", bullets: [] });
    return sections[sections.length - 1];
  };
  for (const node of doc.content ?? []) {
    if (node.type === "heading") {
      sections.push({ heading: plain(node).trim(), bullets: [] });
    } else if (node.type === "bulletList" || node.type === "orderedList" || node.type === "taskList") {
      current().bullets.push(...items(node));
    } else if (node.type === "paragraph") {
      // Texto suelto entre secciones: viñeta del usuario.
      const b = bullet(node, false, false);
      if (b) current().bullets.push(b);
    }
  }
  return { sections: sections.filter((s) => s.heading || s.bullets.length) };
}

// Las sublistas se aplanan: cada elemento anidado pasa a ser una viñeta más.
function items(list: JSONContent): Bullet[] {
  const out: Bullet[] = [];
  for (const item of list.content ?? []) {
    const task = item.type === "taskItem";
    for (const child of item.content ?? []) {
      if (child.type === "paragraph") {
        const b = bullet(child, task, task && !!item.attrs?.checked);
        if (b) out.push(b);
      } else if (child.type === "bulletList" || child.type === "orderedList" || child.type === "taskList") {
        out.push(...items(child));
      }
    }
  }
  return out;
}

function bullet(par: JSONContent, task: boolean, done: boolean): Bullet | null {
  let text = "";
  let user = false;
  let source: Citation | null = null;
  for (const n of par.content ?? []) {
    if (n.type === "text" && n.text) {
      text += n.text;
      if (n.text.trim() && !(n.marks ?? []).some((m) => m.type === "ai")) user = true;
    } else if (n.type === "citation" && !source) {
      source = (n.attrs?.source as Citation | null) ?? null;
    } else if (n.type === "hardBreak") {
      text += " ";
    }
  }
  text = text.replace(/\s+/g, " ").trim();
  if (!text) return null;
  return { text, is_user_text: user, is_action_item: task, done, source };
}

function plain(node: JSONContent): string {
  if (node.type === "text") return node.text ?? "";
  return (node.content ?? []).map(plain).join("");
}
