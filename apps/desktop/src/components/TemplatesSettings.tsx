import { useEffect, useState } from "react";
import { ArrowDown, ArrowUp, Copy, LayoutTemplate, ListChecks, Lock, Pencil, Plus, Trash2, X } from "lucide-react";
import { ipc, type SectionKind, type Template, type TemplateSection } from "@/lib/ipc";
import { tBackend, useI18n, useT } from "@/lib/i18n";
import type { Key } from "@/lib/strings";
import { cn } from "@/lib/utils";
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

// Mismos límites que `Template::validate` (crates/core-types): se comprueban
// aquí para dar el error en el idioma de la interfaz antes de guardar; el
// backend vuelve a validar.
const MAX_SECTIONS = 10;
const MAX_HEADING = 60;
const MAX_HINT = 200;
const MAX_NAME = 80;

/** Como `fold` en Rust: minúsculas, sin tildes ni puntuación, espacios normalizados. */
const fold = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();

export function validateTemplate(tpl: Template): { key: Key; vars?: Record<string, string | number> } | null {
  const name = tpl.name.trim();
  if (!name) return { key: "tpl.err.name" };
  if (name.length > MAX_NAME) return { key: "tpl.err.nameLength", vars: { max: MAX_NAME } };
  const n = tpl.structure.length;
  if (n === 0 || n > MAX_SECTIONS) return { key: "tpl.err.count", vars: { max: MAX_SECTIONS } };
  const seen = new Set<string>();
  for (const [i, s] of tpl.structure.entries()) {
    const h = s.heading.trim();
    if (!h || [...h].length > MAX_HEADING) return { key: "tpl.err.heading", vars: { n: i + 1, max: MAX_HEADING } };
    if (/[#\n\r]/.test(h)) return { key: "tpl.err.headingChars", vars: { h } };
    if (seen.has(fold(h))) return { key: "tpl.err.duplicate", vars: { h } };
    seen.add(fold(h));
    if ([...s.hint.trim()].length > MAX_HINT || /[\n\r]/.test(s.hint)) return { key: "tpl.err.hint", vars: { h, max: MAX_HINT } };
  }
  if (tpl.structure.filter((s) => s.kind === "tasks").length > 1) return { key: "tpl.err.tasks" };
  return null;
}

const section = (heading: string, kind: SectionKind = "topics"): TemplateSection => ({ heading, hint: "", kind });

// Ajustes → Plantillas: las integradas (solo se duplican) y las del usuario
// (crear, editar, duplicar y borrar). El editor sustituye a la lista.
export function TemplatesSettings() {
  const t = useT();
  const uiLang = useI18n((s) => s.lang);
  const [templates, setTemplates] = useState<Template[] | null>(null);
  const [editing, setEditing] = useState<{ tpl: Template; isNew: boolean } | null>(null);
  const [deleting, setDeleting] = useState<Template | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = () =>
    ipc
      .listTemplates(uiLang)
      .then((list) => {
        setTemplates(list);
        setError(null);
      })
      .catch((e) => setError(tBackend(String(e))));
  useEffect(() => {
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [uiLang]);

  const startNew = () =>
    setEditing({
      isNew: true,
      tpl: {
        id: crypto.randomUUID(),
        name: "",
        purpose: "",
        style: "",
        structure: [section(t("tpl.defaultTopics")), section(t("tpl.defaultNextSteps"), "tasks")],
        is_builtin: false,
        shared_with_workspace: false,
      },
    });
  const duplicate = (tpl: Template) =>
    setEditing({
      isNew: true,
      tpl: { ...structuredClone(tpl), id: crypto.randomUUID(), name: t("tpl.copyName", { name: tpl.name }), is_builtin: false },
    });
  const remove = async (tpl: Template) => {
    setDeleting(null);
    try {
      await ipc.deleteTemplate(tpl.id);
      await reload();
    } catch (e) {
      setError(tBackend(String(e)));
    }
  };

  if (editing)
    return (
      <TemplateEditor
        initial={editing.tpl}
        isNew={editing.isNew}
        onCancel={() => setEditing(null)}
        onSaved={async () => {
          setEditing(null);
          await reload();
        }}
      />
    );

  const builtins = templates?.filter((x) => x.is_builtin) ?? [];
  const own = templates?.filter((x) => !x.is_builtin) ?? [];
  return (
    <>
      <div className="mb-8 mt-6 flex items-center gap-4">
        <h1 className="page-title flex-1 font-serif text-[34px] font-normal leading-tight">{t("settings.templates")}</h1>
        <Button className="new-template" size="lg" onClick={startNew}>
          <Plus /> {t("tpl.new")}
        </Button>
      </div>
      <p className="-mt-4 mb-8 text-[14px] leading-snug text-muted">{t("tpl.intro")}</p>
      {error && <div className="note-error mb-4 text-[13.5px] text-danger">{tBackend(error)}</div>}

      <section className="settings-group mb-8">
        <h2 className="mb-2.5 text-[14px] text-muted">{t("tpl.mine")}</h2>
        <div className="divide-y divide-line rounded-[20px] border border-line bg-card">
          {templates && own.length === 0 && <div className="px-5 py-4 text-[14px] text-muted">{t("tpl.none")}</div>}
          {own.map((tpl) => (
            <TemplateRow key={tpl.id} tpl={tpl}>
              <Button size="icon" variant="ghost" aria-label={t("tpl.edit")} title={t("tpl.edit")} onClick={() => setEditing({ tpl: structuredClone(tpl), isNew: false })}>
                <Pencil />
              </Button>
              <Button size="icon" variant="ghost" aria-label={t("tpl.duplicate")} title={t("tpl.duplicate")} onClick={() => duplicate(tpl)}>
                <Copy />
              </Button>
              <Button size="icon" variant="ghost" className="delete-template" aria-label={t("tpl.delete")} title={t("tpl.delete")} onClick={() => setDeleting(tpl)}>
                <Trash2 />
              </Button>
            </TemplateRow>
          ))}
        </div>
      </section>

      <section className="settings-group mb-8">
        <h2 className="mb-2.5 text-[14px] text-muted">{t("tpl.builtin")}</h2>
        <div className="divide-y divide-line rounded-[20px] border border-line bg-card">
          {builtins.map((tpl) => (
            <TemplateRow key={tpl.id} tpl={tpl}>
              <Lock className="size-4 text-faint" aria-label={t("tpl.builtinLocked")} />
              <Button size="sm" className="duplicate-template" onClick={() => duplicate(tpl)}>
                <Copy /> {t("tpl.duplicate")}
              </Button>
            </TemplateRow>
          ))}
        </div>
      </section>

      <AlertDialog open={deleting !== null} onOpenChange={(open) => !open && setDeleting(null)}>
        <AlertDialogContent className="template-confirm">
          <AlertDialogTitle>{t("tpl.deleteTitle", { name: deleting?.name ?? "" })}</AlertDialogTitle>
          <AlertDialogDescription>{t("tpl.deleteDesc")}</AlertDialogDescription>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction onClick={() => deleting && remove(deleting)}>{t("tpl.delete")}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

function TemplateRow({ tpl, children }: { tpl: Template; children: React.ReactNode }) {
  return (
    <div className="template-row flex items-center gap-4 px-5 py-4" data-template-id={tpl.id}>
      <span className="flex size-10 shrink-0 items-center justify-center rounded-xl border border-line bg-canvas text-ink-2 [&_svg]:size-[18px]">
        <LayoutTemplate />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[15px]">{tpl.name}</span>
        <span className="mt-0.5 block truncate text-[13.5px] text-muted">{tpl.structure.map((s) => s.heading).join(" · ")}</span>
      </span>
      <span className="flex shrink-0 items-center gap-1">{children}</span>
    </div>
  );
}

const input =
  "h-9 w-full rounded-xl border border-line bg-surface px-3 text-[14px] outline-none placeholder:text-faint focus:border-ink-2";

function TemplateEditor({ initial, isNew, onCancel, onSaved }: { initial: Template; isNew: boolean; onCancel: () => void; onSaved: () => void }) {
  const t = useT();
  const [tpl, setTpl] = useState(initial);
  // Claves estables de los apartados: con el índice, al reordenar o quitar uno
  // React reutilizaría los inputs (foco y cursor) del apartado equivocado.
  const [keys, setKeys] = useState(() => initial.structure.map(() => crypto.randomUUID()));
  const addSection = (s: TemplateSection) => {
    setKeys((k) => [...k, crypto.randomUUID()]);
    patch({ structure: [...tpl.structure, s] });
  };
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const patch = (p: Partial<Template>) => setTpl((x) => ({ ...x, ...p }));
  const patchSection = (i: number, p: Partial<TemplateSection>) =>
    setTpl((x) => ({ ...x, structure: x.structure.map((s, j) => (j === i ? { ...s, ...p } : s)) }));
  const move = (i: number, d: -1 | 1) => {
    setKeys((k) => {
      const c = [...k];
      [c[i], c[i + d]] = [c[i + d], c[i]];
      return c;
    });
    setTpl((x) => {
      const s = [...x.structure];
      [s[i], s[i + d]] = [s[i + d], s[i]];
      return { ...x, structure: s };
    });
  };
  const hasTasks = tpl.structure.some((s) => s.kind === "tasks");

  const save = async () => {
    const v = validateTemplate(tpl);
    if (v) return setError(t(v.key, v.vars));
    setError(null);
    setSaving(true);
    try {
      await ipc.saveTemplate(tpl);
      onSaved();
    } catch (e) {
      setError(tBackend(String(e)));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="template-editor">
      <h1 className="page-title mb-8 mt-6 font-serif text-[34px] font-normal leading-tight">{isNew ? t("tpl.new") : t("tpl.edit")}</h1>

      <section className="settings-group mb-8">
        <div className="space-y-4 rounded-[20px] border border-line bg-card px-5 py-5">
          <label className="block">
            <span className="mb-1.5 block text-[13.5px] text-muted">{t("tpl.name")}</span>
            <input className={cn(input, "tpl-name")} value={tpl.name} maxLength={MAX_NAME} placeholder={t("tpl.namePlaceholder")} onChange={(e) => patch({ name: e.target.value })} autoFocus />
          </label>
          <label className="block">
            <span className="mb-1.5 block text-[13.5px] text-muted">{t("tpl.purpose")}</span>
            <input className={input} value={tpl.purpose} maxLength={MAX_HINT} placeholder={t("tpl.purposePlaceholder")} onChange={(e) => patch({ purpose: e.target.value })} />
          </label>
          <label className="block">
            <span className="mb-1.5 block text-[13.5px] text-muted">{t("tpl.style")}</span>
            <input className={input} value={tpl.style} maxLength={MAX_HINT} placeholder={t("tpl.stylePlaceholder")} onChange={(e) => patch({ style: e.target.value })} />
          </label>
        </div>
      </section>

      <section className="settings-group mb-6">
        <h2 className="mb-2.5 text-[14px] text-muted">{t("tpl.sections")}</h2>
        <div className="divide-y divide-line rounded-[20px] border border-line bg-card">
          {tpl.structure.map((s, i) => (
            <div key={keys[i] ?? i} className="template-section flex gap-3 px-5 py-4">
              <span className="mt-2 w-5 shrink-0 text-right text-[13px] tabular-nums text-faint">{i + 1}</span>
              <div className="min-w-0 flex-1 space-y-2">
                <div className="flex gap-2">
                  <input
                    className={cn(input, "tpl-heading flex-1")}
                    value={s.heading}
                    maxLength={MAX_HEADING}
                    placeholder={t("tpl.headingPlaceholder")}
                    aria-label={t("tpl.heading")}
                    onChange={(e) => patchSection(i, { heading: e.target.value })}
                  />
                  <select
                    className="tpl-kind h-9 shrink-0 cursor-pointer rounded-xl border border-line bg-surface px-2.5 text-[14px] outline-none"
                    value={s.kind}
                    aria-label={t("tpl.kind")}
                    onChange={(e) => patchSection(i, { kind: e.target.value as SectionKind })}
                  >
                    <option value="topics">{t("tpl.kindTopics")}</option>
                    <option value="tasks" disabled={hasTasks && s.kind !== "tasks"}>
                      {t("tpl.kindTasks")}
                    </option>
                  </select>
                </div>
                <input
                  className={cn(input, "tpl-hint text-[13.5px]")}
                  value={s.hint}
                  maxLength={MAX_HINT}
                  placeholder={s.kind === "tasks" ? t("tpl.hintTasksPlaceholder") : t("tpl.hintPlaceholder")}
                  aria-label={t("tpl.hint")}
                  onChange={(e) => patchSection(i, { hint: e.target.value })}
                />
              </div>
              <div className="flex shrink-0 items-start gap-0.5">
                <Button size="iconSm" variant="ghost" aria-label={t("tpl.moveUp")} disabled={i === 0} onClick={() => move(i, -1)}>
                  <ArrowUp />
                </Button>
                <Button size="iconSm" variant="ghost" aria-label={t("tpl.moveDown")} disabled={i === tpl.structure.length - 1} onClick={() => move(i, 1)}>
                  <ArrowDown />
                </Button>
                <Button
                  size="iconSm"
                  variant="ghost"
                  aria-label={t("tpl.removeSection")}
                  disabled={tpl.structure.length === 1}
                  onClick={() => {
                    setKeys((k) => k.filter((_, j) => j !== i));
                    setTpl((x) => ({ ...x, structure: x.structure.filter((_, j) => j !== i) }));
                  }}
                >
                  <X />
                </Button>
              </div>
            </div>
          ))}
        </div>
        <div className="mt-3 flex gap-2">
          <Button className="add-section" disabled={tpl.structure.length >= MAX_SECTIONS} onClick={() => addSection(section(""))}>
            <Plus /> {t("tpl.addSection")}
          </Button>
          {!hasTasks && (
            <Button disabled={tpl.structure.length >= MAX_SECTIONS} onClick={() => addSection(section(t("tpl.defaultNextSteps"), "tasks"))}>
              <ListChecks /> {t("tpl.addTasks")}
            </Button>
          )}
        </div>
        <p className="mt-3 text-[13px] leading-snug text-muted">{t("tpl.sectionsHelp")}</p>
      </section>

      {error && <div className="note-error mb-4 rounded-xl border border-line px-4 py-2.5 text-[13.5px] text-danger">{error}</div>}
      <div className="flex justify-end gap-2">
        <Button size="lg" onClick={onCancel}>
          {t("common.cancel")}
        </Button>
        <Button size="lg" variant="primary" className="save-template" disabled={saving} onClick={save}>
          {t("tpl.save")}
        </Button>
      </div>
    </div>
  );
}
