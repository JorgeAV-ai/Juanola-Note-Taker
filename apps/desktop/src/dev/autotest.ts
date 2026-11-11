// Recorrido automático de la app (solo con JUANOLA_AUTOTEST=1). Pulsa los
// botones reales del DOM, como haría el usuario, y escribe cada comprobación en
// el log de la app con el prefijo `autotest`:
//   nota nueva → Start → audio del sistema con `say` → Stop → Enhanced →
//   lupa de cita → edición a mano (se guarda como texto del usuario) → tarea →
//   cambio de pestaña → Pause/Resume → segunda grabación en la misma nota →
//   regenerar con la plantilla «Reunión semanal de equipo».
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { events, ipc, type EnhancedNotes, type NoteStatusEvent, type RecordingState } from "../lib/ipc";
import { t } from "../lib/i18n";

const log = (line: string) => invoke("autotest_log", { line }).catch(() => {});
const say = (text: string) => invoke("autotest_say", { text, voice: "Mónica" });
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

let failures = 0;
function check(ok: boolean, what: string, detail = "") {
  lastStep = what;
  if (!ok) failures++;
  log(`${ok ? "OK  " : "FAIL"} ${what}${detail ? ` — ${detail}` : ""}`);
  return ok;
}

async function waitFor<T>(what: string, fn: () => T | null | undefined | false, timeoutMs: number): Promise<T | null> {
  const t0 = performance.now();
  while (performance.now() - t0 < timeoutMs) {
    const v = fn();
    if (v) {
      log(`     ${what} (${((performance.now() - t0) / 1000).toFixed(1)} s)`);
      return v;
    }
    await sleep(200);
  }
  check(false, what, `no pasó en ${timeoutMs / 1000} s`);
  return null;
}

const buttons = (sel: string) => Array.from(document.querySelectorAll<HTMLButtonElement>(sel));
const button = (sel: string, text: string) => buttons(sel).find((b) => b.textContent?.trim() === text && !b.disabled) ?? null;

// Los menús de Radix se abren con pointerdown, no con click().
const press = (el: Element | null) =>
  el?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, cancelable: true, button: 0, pointerType: "mouse" }));
async function menuItem(trigger: string, text: string): Promise<boolean> {
  const find = () => Array.from(document.querySelectorAll<HTMLElement>('[role="menuitem"]')).find((e) => e.textContent?.includes(text));
  // Recién montado, el primer pointerdown puede llegar antes de que Radix
  // esté listo: se reintenta hasta tres veces.
  for (let i = 0; i < 3 && !find(); i++) {
    press(document.querySelector(trigger));
    for (let j = 0; j < 5 && !find(); j++) await sleep(100);
  }
  const it = await waitFor(`menú «${text}»`, find, 2000);
  it?.click();
  await sleep(200);
  return !!it;
}
// ✨: botón «Enhance notes» si aún no hay notas mejoradas; si las hay, menú de Enhanced.
async function regenerate() {
  const b = document.querySelector<HTMLButtonElement>(`button[title="${t("note.generate")}"]:not([disabled])`);
  if (b) b.click();
  else await menuItem(".enhance-menu", t("note.regenerate"));
}

const SESSION_1 =
  "Buenos días a todos. Empezamos la reunión semanal de producto. Las ventas de octubre han subido un doce por ciento " +
  "y ya tenemos mil quinientos clientes activos. Laura se encarga de preparar la presentación para el viernes. " +
  "Carlos revisará los errores de la aplicación móvil antes del lunes. El presupuesto de marketing queda en " +
  "treinta mil euros para el próximo trimestre. Decidimos lanzar la nueva versión el veinte de noviembre.";
const SESSION_2 =
  "Una cosa más antes de terminar. El equipo de soporte ha cerrado cuatrocientas incidencias este mes. " +
  "Marta enviará el informe de satisfacción a todos el miércoles. Nos vemos la semana que viene.";

// Vigía: si el hilo principal se queda parado más de 1 s, lo deja en el log
// (con lo último que hizo el autotest), para localizar bloqueos de la interfaz.
let lastStep = "inicio";
function watchdog() {
  let last = performance.now();
  setInterval(() => {
    const now = performance.now();
    if (now - last > 1500) log(`BLOQUEO de ${((now - last) / 1000).toFixed(1)} s (tras «${lastStep}»)`);
    last = now;
  }, 250);
}

export async function runAutotest(mode: string) {
  watchdog();
  log(`inicio (modo ${mode})`);
  try {
    if (mode === "enhance") await enhanceOnly();
    else if (mode === "ui") await sidebarFlow();
    else if (mode === "detect") await detectFlow();
    else if (mode === "detect-live") await detectLiveFlow();
    else if (mode === "detect-calendar") await detectCalendarFlow();
    else if (mode === "context") await contextFlow();
    else await flow();
  } catch (e) {
    check(false, "excepción", String(e));
  }
  log(failures ? `FIN: ${failures} fallos` : "FIN: todo OK");
  await sleep(500);
  invoke("autotest_quit").catch(() => {});
}

// JUANOLA_AUTOTEST=context: graba dos veces el WAV de JUANOLA_AUTOTEST_AUDIO
// (por los altavoces, canal del sistema), primero sin vocabulario y luego con
// el de JUANOLA_AUTOTEST_GLOSSARY, y deja cada transcripción en el log como
// `SEGMENTOS <etiqueta> <json>`. Restaura el vocabulario y manda las notas a
// la papelera al acabar.
async function contextFlow() {
  const glossary = await invoke<string | null>("autotest_env", { key: "JUANOLA_AUTOTEST_GLOSSARY" });
  const title = (await invoke<string | null>("autotest_env", { key: "JUANOLA_AUTOTEST_TITLE" })) ?? "Autotest contexto";
  const before = await ipc.sttGlossary();
  try {
    for (const [label, terms] of [["sin", ""], ["con", glossary ?? ""]] as const) {
      await ipc.setSttGlossary(terms);
      const newNote = await waitFor("botón New note", () => document.querySelector<HTMLButtonElement>(".new-note"), 15_000);
      newNote?.click();
      const noteEl = await waitFor("vista de nota", () => document.querySelector<HTMLElement>(".note[data-note-id]"), 10_000);
      if (!noteEl) return;
      const id = noteEl.dataset.noteId!;
      await ipc.setNoteTitle(id, title);
      await sleep(500);
      button(".pill-btn", t("rec.start"))?.click();
      const rec = await waitFor(
        `grabando (${label})`,
        () => (button(".pill-btn", t("rec.pause")) ? "ok" : document.querySelector(".pill-msg.error") ? "error" : null),
        150_000,
      );
      if (!check(rec === "ok", `arranque de la grabación (${label})`, document.querySelector(".pill-msg")?.textContent ?? "")) return;
      // Con auriculares Bluetooth, abrir el audio del sistema tarda más de 6 s
      // y se reintenta en segundo plano: margen antes de que empiece a sonar.
      await sleep(12_000);
      await invoke("autotest_play");
      await sleep(8000);
      button(".pill-btn.stop", t("rec.stop"))?.click();
      // Esperar a que el enhance de después de parar empiece y acabe: si no,
      // la siguiente grabación compite con él por la GPU.
      await waitFor(`enhance empezado (${label})`, () => document.querySelector(".enhancing") || document.querySelector(".note-error"), 60_000);
      await waitFor(
        `enhance terminado (${label})`,
        () => !document.querySelector(".enhancing") && (document.querySelector(".enhanced .ProseMirror") || document.querySelector(".note-error")),
        300_000,
      );
      const enhErr = document.querySelector(".note-error");
      if (enhErr) check(false, `enhance sin error (${label})`, enhErr.textContent ?? "");
      const segs = await ipc.listSegments(id);
      check(segs.some((s) => s.channel === "system"), `segmentos del sistema (${label})`, `${segs.length} segmentos`);
      log(`SEGMENTOS ${label} ${JSON.stringify(segs)}`);
      // Volver a Home antes de tirar la nota: «New note» solo está fuera de una nota.
      document.querySelector<HTMLButtonElement>(".back-home")?.click();
      await waitFor(`de vuelta en Home (${label})`, () => document.querySelector(".new-note"), 10_000);
      await ipc.trashNote(id);
      await sleep(1000);
    }
  } finally {
    await ipc.setSttGlossary(before);
  }
}

async function flow() {
  // 1. Nota nueva.
  const newNote = await waitFor("botón New note", () => document.querySelector<HTMLButtonElement>(".new-note"), 15_000);
  if (!newNote) return;
  newNote.click();
  const noteEl = await waitFor("vista de nota", () => document.querySelector<HTMLElement>(".note[data-note-id]"), 10_000);
  if (!noteEl) return;
  const id = noteEl.dataset.noteId!;
  await ipc.setNoteTitle(id, `Autotest ${new Date().toLocaleString()}`);
  // El idioma está en el pie del panel del transcript: se despliega.
  if (!document.querySelector(".pill-lang")) document.querySelector<HTMLButtonElement>(`button[aria-label="${t("note.transcript")}"]`)?.click();
  await sleep(300);
  const lang = document.querySelector<HTMLSelectElement>(".pill-lang");
  let stored: string | null = null;
  try {
    stored = localStorage.getItem("juanola.language");
  } catch {
    /* sin almacenamiento */
  }
  check(!!lang && lang.value === (stored ?? "Spanish"), "selector de idioma con la preferencia guardada (o Español)", `${lang?.value}; guardado=${stored}; opciones=${Array.from(lang?.options ?? []).map((o) => o.value).join(",")}`);

  // 2. Primera grabación con audio del sistema.
  button(".pill-btn", t("rec.start"))?.click();
  const rec = await waitFor(
    "grabando (o error)",
    () => (button(".pill-btn", t("rec.pause")) ? "ok" : document.querySelector(".pill-msg.error") ? "error" : null),
    150_000,
  );
  if (rec !== "ok") {
    check(false, "arranque de la grabación", document.querySelector(".pill-msg")?.textContent ?? "");
    return;
  }
  await sleep(1500);
  await say(SESSION_1);
  const them = await waitFor("frase final de Them en el drawer", () => document.querySelector(".bubble.them:not(.partial)"), 8_000);
  await sleep(2500);
  const themText = Array.from(document.querySelectorAll(".bubble.them:not(.partial)")).map((b) => b.textContent).join(" ");
  check(!!them && /doce|12|presupuesto|noviembre/i.test(themText), "transcripción del sistema", themText.slice(0, 160));

  // Pause / Resume dentro de la sesión.
  button(".pill-btn", t("rec.pause"))?.click();
  check(!!(await waitFor("en pausa", () => document.querySelector(".pill-label")?.textContent === t("rec.paused"), 5_000)), t("rec.pause"));
  button(".pill-btn", t("rec.resume"))?.click();
  check(!!(await waitFor("reanudada", () => button(".pill-btn", t("rec.pause")), 5_000)), "Resume tras Pause");
  await sleep(2000);

  button(".pill-btn.stop", t("rec.stop"))?.click();
  const enhanced = await waitFor(
    "Enhanced generado (o error)",
    () => document.querySelector(".enhanced .ProseMirror") || document.querySelector(".note-error"),
    180_000,
  );
  const enhErr = document.querySelector(".note-error");
  if (enhErr) check(false, "enhance sin error", enhErr.textContent ?? "");
  if (!check(!!enhanced, "notas mejoradas visibles")) return;
  const segs1 = await ipc.listSegments(id);
  check(segs1.some((s) => s.channel === "system"), "segmentos del sistema guardados", `${segs1.length} segmentos`);

  if (!(await checkEnhanced(id))) return;

  // 7. Segunda grabación en la misma nota (el fallo de antes).
  const resume = await waitFor("botón Resume de la píldora", () => button(".pill-btn", t("rec.resume")), 10_000);
  resume?.click();
  const rec2 = await waitFor(
    "segunda sesión grabando (o error)",
    () => (button(".pill-btn", t("rec.pause")) ? "ok" : document.querySelector(".pill-msg.error") ? "error" : null),
    150_000,
  );
  if (rec2 !== "ok") {
    check(false, "arranque de la segunda grabación", document.querySelector(".pill-msg")?.textContent ?? "");
    return;
  }
  await sleep(1500);
  await say(SESSION_2);
  await sleep(3000);
  button(".pill-btn.stop", t("rec.stop"))?.click();
  // Las notas están editadas: al parar no se regeneran sin preguntar.
  const ask = await waitFor("aviso al parar con notas editadas", () => button(".note-confirm button", t("note.regenerate")), 120_000);
  check(!!ask, "al parar, pregunta antes de regenerar las notas editadas");
  ask?.click();
  await waitFor("segundo enhance empezado", () => document.querySelector(".enhancing"), 10_000);
  await waitFor("segundo enhance terminado", () => !document.querySelector(".enhancing") && document.querySelector(".enhanced .ProseMirror"), 180_000);
  await sleep(1000);
  const segs2 = await ipc.listSegments(id);
  const last1 = Math.max(...segs1.map((s) => s.end));
  const newer = segs2.filter((s) => s.start > last1);
  check(newer.some((s) => s.channel === "system"), "segunda sesión transcrita", newer.map((s) => s.text).join(" ").slice(0, 160));
  const err = document.querySelector(".note-error, .pill-msg.error");
  check(!err, "sin errores en pantalla", err?.textContent ?? "");

  await templateStep(id);
}

// Id fijo de «Reunión semanal de equipo» (crates/enhance/src/templates.rs).
const WEEKLY_TEAM = "6a75616e-6f6c-4100-8000-000000000003";
// Apartados que añade el pipeline fuera de la plantilla.
const EXTRA_HEADINGS = new Set(["Otros temas", "Other topics", "Tus notas", "Your notes"]);

// Regenera la nota con la plantilla desde el submenú «Plantilla» y comprueba
// que los títulos siguen la plantilla, en su orden, y que no hubo caída a Auto.
async function templateStep(id: string) {
  let ready: NoteStatusEvent | null = null;
  const un = await events.noteStatus((e) => {
    if (e.note_id === id && e.status !== "enhancing") ready = e;
  });
  try {
    press(document.querySelector(".enhance-menu"));
    const sub = await waitFor("submenú Plantilla", () => document.querySelector<HTMLElement>(".template-menu"), 3000);
    sub?.click();
    const item = await waitFor("plantilla en el submenú", () => document.querySelector<HTMLElement>(`[data-template-id="${WEEKLY_TEAM}"]`), 3000);
    if (!check(!!item, "submenú con «Reunión semanal de equipo»")) return;
    item!.click();
    await sleep(400);
    button(".note-confirm button", t("note.regenerate"))?.click();
    await waitFor("enhance con plantilla empezado", () => document.querySelector(".enhancing") || document.querySelector(".note-error"), 10_000);
    const t0 = performance.now();
    await waitFor("enhance con plantilla terminado", () => (!document.querySelector(".enhancing") && ready) || document.querySelector(".note-error"), 240_000);
    const err = document.querySelector(".note-error");
    // `ready` lo asigna el listener: TypeScript no lo ve y lo deja en `null`.
    const got = ready as NoteStatusEvent | null;
    if (!check(!err && got?.status === "ready", "enhance con plantilla sin error", err?.textContent ?? `${((performance.now() - t0) / 1000).toFixed(1)} s`)) return;
    check(got?.notice !== "template_fallback", "plantilla aplicada sin caer a Auto");
    check(!!document.querySelector(".note-notice") === (got?.notice === "template_fallback"), "aviso de caída a Auto solo si la hubo");

    const saved = await ipc.getNote(id);
    check(saved?.template_id === WEEKLY_TEAM, "plantilla guardada en la nota", String(saved?.template_id));
    const headings = (saved?.enhanced_doc?.sections ?? []).map((s) => s.heading);
    // Los títulos de la plantilla van en el idioma del resumen: se prueba con los dos.
    const options = await Promise.all(["español", "inglés"].map((l) => ipc.listTemplates(l)));
    const expected = options.map((list) => list.find((x) => x.id === WEEKLY_TEAM)?.structure.map((s) => s.heading) ?? []);
    const own = headings.filter((h) => !EXTRA_HEADINGS.has(h));
    const ordered = expected.some((exp) => {
      let i = 0;
      return own.every((h) => {
        while (i < exp.length && exp[i] !== h) i++;
        return i++ < exp.length;
      });
    });
    check(own.length > 0 && ordered, "títulos: subconjunto ordenado de la plantilla", headings.join(" · "));
    check(expected.some((exp) => exp.every((h) => own.includes(h))), "todos los apartados de la plantilla presentes", headings.join(" · "));
    // El menú marca la plantilla actual.
    press(document.querySelector(".enhance-menu"));
    (await waitFor("submenú Plantilla", () => document.querySelector<HTMLElement>(".template-menu"), 3000))?.click();
    const marked = await waitFor("plantilla marcada", () => document.querySelector(`[data-template-id="${WEEKLY_TEAM}"] svg`), 3000);
    check(!!marked, "el submenú marca la plantilla de la nota");
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    await sleep(300);
  } finally {
    un();
  }
}

// Solo enhance + editor sobre la nota transcrita más reciente (sin audio: no
// necesita los permisos de micrófono ni de audio del sistema).
async function enhanceOnly() {
  await waitFor("lista de notas", () => document.querySelector(".row[data-note-id]"), 15_000);
  let id: string | null = null;
  for (const n of await ipc.listNotes()) {
    const segs = await ipc.listSegments(n.id);
    if (segs.map((x) => x.text).join(" ").split(/\s+/).length >= 50) {
      id = n.id;
      break;
    }
  }
  if (!check(!!id, "hay una nota con transcripción")) return;
  document.querySelector<HTMLElement>(`.row[data-note-id="${id}"]`)?.click();
  const gen = await waitFor("botón ✨", () => document.querySelector(".note-tabs"), 10_000);
  if (gen) await regenerate();
  await sleep(400);
  button(".note-confirm button", t("note.regenerate"))?.click();
  const t0 = performance.now();
  await waitFor("enhance en marcha", () => document.querySelector(".enhancing") || document.querySelector(".note-error"), 10_000);
  const done = await waitFor(
    "Enhanced generado (o error)",
    () => (!document.querySelector(".enhancing") && document.querySelector(".enhanced .ProseMirror")) || document.querySelector(".note-error"),
    180_000,
  );
  const err = document.querySelector(".note-error");
  if (!check(!!done && !err, "enhance sin error", err?.textContent ?? `${((performance.now() - t0) / 1000).toFixed(1)} s`)) return;
  await checkEnhanced(id!);
}

async function checkEnhanced(id: string): Promise<boolean> {
  // 3. Contenido del Enhanced: texto de IA en gris, citas, lupa.
  const ai = document.querySelectorAll(".enhanced .ai-text").length;
  const cites = buttons(".enhanced .cite");
  check(ai > 0, "texto de la IA marcado en gris", `${ai} tramos`);
  check(cites.length > 0, "viñetas con cita", `${cites.length}`);
  if (cites[0]) {
    cites[0].click();
    const pop = await waitFor("lupa abre el fragmento", () => document.querySelector(".cite-pop .cite-line"), 3_000);
    check(!!pop, "fragmento citado con texto", pop?.textContent?.slice(0, 100) ?? "");
    cites[0].click();
  }

  // 4. Edición a mano: lo escrito pasa a texto del usuario y se guarda.
  const firstPar = document.querySelector<HTMLElement>(".enhanced li p");
  if (firstPar) {
    const pm = document.querySelector<HTMLElement>(".enhanced .ProseMirror")!;
    pm.focus();
    const range = document.createRange();
    const lastText = Array.from(firstPar.childNodes).filter((n) => n.textContent).pop()!;
    range.selectNodeContents(lastText);
    range.collapse(false);
    const sel = window.getSelection()!;
    sel.removeAllRanges();
    sel.addRange(range);
    document.execCommand("insertText", false, " EDITADO");
    await sleep(300);
    if (!document.querySelector(".enhanced")?.textContent?.includes("EDITADO")) {
      // WKWebView sin foco (app en segundo plano) ignora execCommand: se inserta
      // con una transacción de TipTap, el mismo camino que una tecla.
      type Ed = { state: { doc: { resolve: (p: number) => unknown } }; view: { posAtDOM: (n: globalThis.Node, o: number) => number }; chain: () => { insertContentAt: (p: number, c: string) => { run: () => boolean } } };
      const ed = (pm as unknown as { editor?: Ed }).editor;
      if (ed) {
        const pos = ed.view.posAtDOM(lastText, lastText.textContent!.length);
        ed.chain().insertContentAt(pos, " EDITADO").run();
        log("     (texto insertado con una transacción de TipTap: execCommand no funciona sin foco)");
      }
      await sleep(300);
    }
    const inAi = Array.from(document.querySelectorAll(".enhanced .ai-text")).some((s) => s.textContent?.includes("EDITADO"));
    check(document.querySelector(".enhanced")?.textContent?.includes("EDITADO") === true, "texto escrito en el editor");
    check(!inAi, "lo escrito no queda en gris");
    await sleep(1200);
    const saved = (await ipc.getNote(id))?.enhanced_doc as EnhancedNotes | null;
    const b = saved?.sections.flatMap((s) => s.bullets).find((x) => x.text.includes("EDITADO"));
    check(!!b, "edición guardada en la base de datos", b?.text);
    check(!!b?.is_user_text, "viñeta editada guardada como texto del usuario");
    check(!!b?.source, "la viñeta editada conserva su cita");
  } else {
    check(false, "hay viñetas para editar");
  }

  // 5. Tarea: marcarla se guarda.
  const box = document.querySelector<HTMLInputElement>('.enhanced ul[data-type="taskList"] input[type="checkbox"]');
  if (box) {
    const before = box.checked;
    box.click();
    await sleep(1200);
    const saved = (await ipc.getNote(id))?.enhanced_doc as EnhancedNotes | null;
    const first = saved?.sections.flatMap((s) => s.bullets).find((x) => x.is_action_item);
    check(!!first && !!first.done === !before, `tarea ${before ? "desmarcada" : "marcada"} y guardada`);
  } else {
    log("     (sin tareas en este resumen; no se prueba el checkbox)");
  }

  // 6. Cambiar de pestaña y volver: la edición sigue.
  button(".tab", t("nav.myNotes"))?.click();
  await sleep(400);
  button(".tab", t("note.enhanced"))?.click();
  await sleep(400);
  check(document.querySelector(".enhanced")?.textContent?.includes("EDITADO") === true, "edición conservada tras cambiar de pestaña");

  // 7. ✨ sobre notas editadas pregunta antes; «Mantener» no toca nada.
  check((await ipc.getNote(id))?.enhanced_doc?.edited === true, "notas marcadas como editadas");
  await regenerate();
  const ask = await waitFor("aviso antes de regenerar", () => document.querySelector(".note-confirm"), 3000);
  check(!!ask && !document.querySelector(".enhancing"), "✨ pregunta antes de pisar las ediciones", ask?.textContent ?? "");
  button(".note-confirm button", t("note.keep"))?.click();
  await sleep(300);
  check(
    !document.querySelector(".note-confirm") && document.querySelector(".enhanced")?.textContent?.includes("EDITADO") === true,
    "«Mantener» conserva las ediciones",
  );

  return true;
}

// Sidebar contra la base de datos real: carpeta, nota dentro, papelera,
// restaurar, renombrar, Ajustes y limpieza (no deja nada).
async function sidebarFlow() {
  const name = `Autotest ${Date.now() % 100000}`;
  const plus = await waitFor("botón de nueva carpeta", () => document.querySelector<HTMLButtonElement>(`button[title="${t("nav.newFolder")}"]`), 15_000);
  plus?.click();
  const input = await waitFor("campo de nombre", () => document.querySelector<HTMLInputElement>(".folder-input"), 2000);
  if (!check(!!input, "el + abre el campo de nombre") || !input) return;
  input.value = name;
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  const item = await waitFor("carpeta en el sidebar", () => buttons(".sidebar-item.folder").find((b) => b.textContent?.includes(name)), 3000);
  check(!!item, "carpeta creada en el sidebar", name);
  const folderId = item?.dataset.folderId;
  check(!!(await waitFor("vista de carpeta", () => document.querySelector<HTMLInputElement>(`input[aria-label="${t("nav.folderName")}"]`)?.value === name || null, 2000)), "se abre la vista de la carpeta");

  // Nota nueva dentro de la carpeta.
  document.querySelector<HTMLButtonElement>(".new-note")?.click();
  const noteEl = await waitFor("vista de nota", () => document.querySelector<HTMLElement>(".note[data-note-id]"), 5000);
  if (!noteEl || !folderId) return;
  const noteId = noteEl.dataset.noteId!;
  await ipc.setNoteTitle(noteId, "Autotest UI");
  await waitFor("selector de carpeta", () => document.querySelector<HTMLElement>(".note-folder")?.dataset.folder === folderId || null, 3000);
  check(document.querySelector<HTMLElement>(".note-folder")?.dataset.folder === folderId, "la nota nueva está en la carpeta");
  const count = () => document.querySelector(`.sidebar-item.folder[data-folder-id="${folderId}"] .count`)?.textContent ?? "0";
  // En la nota no hay sidebar: se vuelve a Home para verlo.
  check(!document.querySelector(".sidebar") && !!document.querySelector(".back-home"), "la nota ocupa toda la ventana, con «‹ ⌂»");
  document.querySelector<HTMLButtonElement>(".back-home")?.click();
  await waitFor("sidebar de vuelta", () => document.querySelector(".sidebar"), 3000);
  check(count() === "1", "recuento de la carpeta", count());
  document.querySelector<HTMLElement>(`.row[data-note-id="${noteId}"]`)?.click();
  await waitFor("nota abierta otra vez", () => document.querySelector(".note-more"), 3000);

  // Papelera y restaurar.
  await menuItem(".note-more", t("note.moveToTrash"));
  await waitFor("vuelta a Home", () => document.querySelector(".coming-up"), 3000);
  await sleep(300);
  check(!document.querySelector(`.row[data-note-id="${noteId}"]`) && count() === "0", "en la papelera: fuera de Home y del recuento");
  (await waitFor("botón Papelera", () => document.querySelector<HTMLButtonElement>(".nav-trash"), 3000))?.click();
  const row = await waitFor("fila en la papelera", () => document.querySelector(`.row[data-note-id="${noteId}"]`), 3000);
  check(!!row, "la nota aparece en la papelera");
  row?.querySelector<HTMLButtonElement>(".restore")?.click();
  await sleep(500);
  check(!document.querySelector(`.row[data-note-id="${noteId}"]`) && count() === "1", "restaurada: vuelve a su carpeta");

  // Renombrar.
  document.querySelector<HTMLButtonElement>(`.sidebar-item.folder[data-folder-id="${folderId}"]`)?.click();
  const title = await waitFor("nombre editable", () => document.querySelector<HTMLInputElement>(`input[aria-label="${t("nav.folderName")}"]`), 3000);
  if (title) {
    title.value = `${name} bis`;
    title.dispatchEvent(new FocusEvent("blur"));
    title.dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
  }
  const renamed = await waitFor("nombre nuevo", () => buttons(".sidebar-item.folder").find((b) => b.textContent?.includes(`${name} bis`)), 3000);
  check(!!renamed, "carpeta renombrada");

  // Ajustes.
  document.querySelector<HTMLButtonElement>(".nav-settings")?.click();
  (await waitFor("Ajustes abiertos", () => button(".settings-nav", t("settings.audio")), 3000))?.click();
  const mic = await waitFor("estado del micrófono", () => {
    const t = document.querySelector('[data-testid="mic-permission"]')?.textContent;
    return t && t !== "…" ? t : null;
  }, 5000);
  check(!!mic, "Ajustes muestra el permiso de micrófono", mic ?? "");
  const devs = await waitFor("dispositivos", () => document.querySelectorAll(".settings-list li").length || null, 5000);
  check(!!devs, "Ajustes lista los dispositivos de entrada", String(devs ?? 0));

  const openMic = buttons(".settings-row button").find((b) => b.textContent === t("common.openSettings"));
  openMic?.click();
  await sleep(1500);
  check(!document.querySelector(".note-error"), "«Abrir Ajustes» del micrófono sin error");
  log("     (Ajustes del Sistema debería estar abierto en Privacidad › Micrófono)");
  button(".settings-nav", t("settings.models"))?.click();
  const rows = await waitFor("modelos", () => document.querySelectorAll(".model-row").length || null, 5000);
  const installed = document.querySelectorAll('.model-row[data-installed="true"]').length;
  check(!!rows && installed === rows, "Ajustes › Models: todos instalados, sin descargas", `${installed}/${rows ?? 0}`);
  check(!document.querySelector(".models-dialog"), "no se ofrece descargar modelos al arrancar");
  button(".settings-nav", t("settings.data"))?.click();
  const dir = await waitFor("carpeta de datos", () => (document.querySelector(".settings .mono")?.textContent?.includes("/") ? true : null), 3000);
  check(!!dir, "Ajustes muestra la carpeta de datos");

  // Limpieza: nota a la papelera y borrada para siempre; carpeta borrada.
  await ipc.trashNote(noteId);
  document.querySelector<HTMLButtonElement>(".back-home")?.click();
  await waitFor("fuera de Ajustes", () => document.querySelector(".nav-trash"), 3000);
  document.querySelector<HTMLButtonElement>(".nav-trash")?.click();
  const trashed = await waitFor("fila a borrar", () => document.querySelector(`.row[data-note-id="${noteId}"]`), 3000);
  document.querySelector<HTMLButtonElement>(`.row[data-note-id="${noteId}"] .destroy`)?.click();
  await sleep(300);
  check(!!document.querySelector(".note-confirm"), "borrar para siempre pide confirmación");
  button(".note-confirm button", t("trash.deleteForever"))?.click();
  await sleep(500);
  check(!!trashed && !(await ipc.getNote(noteId)), "nota borrada para siempre");
  document.querySelector<HTMLButtonElement>(`.sidebar-item.folder[data-folder-id="${folderId}"]`)?.click();
  await waitFor("botón borrar carpeta", () => document.querySelector(".delete-folder"), 3000);
  document.querySelector<HTMLButtonElement>(".delete-folder")?.click();
  await sleep(300);
  button(".note-confirm button", t("folder.delete"))?.click();
  await sleep(500);
  check(!document.querySelector(`.sidebar-item.folder[data-folder-id="${folderId}"]`), "carpeta borrada");
}

// JUANOLA_AUTOTEST=detect: avisos de reunión y calendario. Muestra un aviso de
// llamada de prueba, pulsa «Take Notes» (abre una nota y empieza a grabar),
// simula que la llamada termina (debe parar sola) y prueba cerrar un aviso de
// calendario. Con JUANOLA_AUTOTEST_NO_MIC=1 no necesita micrófono.
async function detectFlow() {
  const settings = await ipc.detectionSettings();
  check(typeof settings.meetings === "boolean", "ajustes de detección", JSON.stringify(settings));
  let access = await ipc.calendarAccess();
  if (access === "not_determined") {
    log("     pidiendo permiso de calendario (hay que pulsar «Permitir»)…");
    access = await ipc.requestCalendarAccess();
  }
  check(access === "granted", "permiso de calendario", access);
  for (const line of await invoke<string[]>("calendar_diagnostics")) log(`     ${line}`);
  const upcoming = access === "granted" ? await ipc.comingUp() : [];
  log(`     calendario: ${access}, ${upcoming.length} eventos próximos`);
  for (const e of upcoming.slice(0, 8)) log(`     evento: ${e.start} · ${e.title} · ${e.attendees.length} asist. · ${e.conference_url ?? "sin enlace"}`);
  // Lo que pinta «Coming up» en Home (el autotest arranca en Home).
  await waitFor("Coming up cargado", () => document.querySelector(".coming-up"), 5000);
  await sleep(500);
  const shown = Array.from(document.querySelectorAll(".coming-event")).map((b) => b.textContent?.trim());
  check(shown.length === Math.min(5, upcoming.length), "Coming up pinta los eventos", JSON.stringify(shown));
  if (upcoming[0]) log(`     primero: ${upcoming[0].title} · ${upcoming[0].start} · ${upcoming[0].attendees.length} asistentes · ${upcoming[0].conference_url ?? "sin enlace"}`);

  let state: RecordingState | null = null;
  const un = events.recordingState((e) => {
    if (e.state !== "warning") state = e.state;
  });
  try {
    await ipc.simulateDetection("adhoc");
    await sleep(1500);
    const payload = await ipc.notificationPayload();
    check(payload?.title === "Meeting detected" && payload.body === "Zoom", "aviso «Meeting detected · Zoom»", JSON.stringify(payload));

    const before = document.querySelector<HTMLElement>(".note[data-note-id]")?.dataset.noteId ?? null;
    await ipc.notificationTakeNotes();
    const noteEl = await waitFor("nota abierta por «Take Notes»", () => {
      const el = document.querySelector<HTMLElement>(".note[data-note-id]");
      return el && el.dataset.noteId !== before ? el : null;
    }, 5000);
    check(!!noteEl, "«Take Notes» abre una nota nueva");
    check((await ipc.notificationPayload()) === null, "el aviso se cierra al pulsar «Take Notes»");
    const noteId = noteEl?.dataset.noteId;
    if (noteId) await ipc.setNoteTitle(noteId, `Autotest detect ${new Date().toLocaleString()}`);

    const started = await waitFor("grabación en marcha", () => (state === "recording" ? true : state === "error" ? "error" : null), 30_000);
    check(started === true, "la grabación empieza sola", String(state));
    if (started === true) {
      await sleep(3000);
      // La app de la llamada suelta el micrófono → debe parar y lanzar el enhance.
      await emit("detect://meeting-ended", { note_id: noteId, app: { bundle_id: "us.zoom.xos", name: "Zoom", kind: "zoom" } });
      const stopped = await waitFor("grabación parada al terminar la llamada", () => (state === "stopped" ? true : null), 20_000);
      check(!!stopped, "al terminar la llamada la grabación para sola");
    } else {
      await ipc.stopRecording().catch(() => {});
    }

    await ipc.simulateDetection("calendar");
    await sleep(1200);
    const cal = await ipc.notificationPayload();
    check(cal?.kind === "calendar" && cal.body.startsWith("Starts in"), "aviso de calendario", JSON.stringify(cal && { title: cal.title, body: cal.body }));
    await ipc.notificationDismiss();
    check((await ipc.notificationPayload()) === null, "× cierra el aviso");

    // Limpieza: la nota de prueba a la papelera cuando termine el enhance.
    if (noteId) {
      await sleep(1000);
      await waitFor("enhance terminado", () => !document.querySelector(".enhancing") || null, 120_000);
      await ipc.trashNote(noteId).catch((e) => log(`     no se pudo borrar la nota: ${e}`));
    }
  } finally {
    un.then((f) => f());
  }
}

// JUANOLA_AUTOTEST=detect-live: espera un aviso real (el script abre Chrome
// con una página que pide el micrófono), pulsa «Take Notes» y espera a que la
// grabación pare sola cuando Chrome suelta el micrófono.
async function detectLiveFlow() {
  let state: RecordingState | null = null;
  const un = events.recordingState((e) => {
    if (e.state !== "warning") state = e.state;
  });
  try {
    log("     esperando un aviso real…");
    let payload = null;
    const t0 = performance.now();
    while (!payload && performance.now() - t0 < 90_000) {
      payload = await ipc.notificationPayload();
      if (!payload) await sleep(500);
    }
    check(!!payload, "aviso real", JSON.stringify(payload && { title: payload.title, body: payload.body, app: payload.app?.bundle_id }));
    if (!payload) return;
    await sleep(4000); // tiempo para verlo en pantalla
    await ipc.notificationTakeNotes();
    const noteEl = await waitFor("nota abierta", () => document.querySelector<HTMLElement>(".note[data-note-id]"), 5000);
    const noteId = noteEl?.dataset.noteId;
    if (noteId) await ipc.setNoteTitle(noteId, `Autotest detect-live ${new Date().toLocaleString()}`);
    const started = await waitFor("grabación en marcha", () => (state === "recording" ? true : state === "error" ? "error" : null), 30_000);
    check(started === true, "la grabación empieza sola", String(state));
    log("     esperando a que Chrome suelte el micrófono…");
    const stopped = await waitFor("grabación parada al cerrar Chrome", () => (state === "stopped" ? true : null), 90_000);
    check(!!stopped, "al soltar el micrófono la grabación para sola");
    if (noteId) {
      await sleep(1000);
      await waitFor("enhance terminado", () => !document.querySelector(".enhancing") || null, 120_000);
      await ipc.trashNote(noteId).catch(() => {});
    }
  } finally {
    un.then((f) => f());
  }
}

// JUANOLA_AUTOTEST=detect-calendar: «Take Notes» en un aviso de calendario y,
// con la grabación en marcha, el script abre un Chrome que coge el micrófono y
// luego lo suelta. La grabación debe asociarse a Chrome y parar sola.
async function detectCalendarFlow() {
  let state: RecordingState | null = null;
  const un = events.recordingState((e) => {
    if (e.state !== "warning") state = e.state;
  });
  try {
    await ipc.simulateDetection("calendar");
    await sleep(1500);
    const payload = await ipc.notificationPayload();
    check(payload?.kind === "calendar", "aviso de calendario", JSON.stringify(payload && { title: payload.title, body: payload.body }));
    if (!payload) return;
    await ipc.notificationTakeNotes();
    const noteEl = await waitFor("nota abierta", () => document.querySelector<HTMLElement>(".note[data-note-id]"), 5000);
    const noteId = noteEl?.dataset.noteId;
    if (noteId) await ipc.setNoteTitle(noteId, `Autotest detect-calendar ${new Date().toLocaleString()}`);
    const started = await waitFor("grabación en marcha", () => (state === "recording" ? true : state === "error" ? "error" : null), 30_000);
    check(started === true, "la grabación empieza sola", String(state));
    if (started !== true) return;
    log("     LISTO: abrid la llamada (Chrome con micrófono)");
    const stopped = await waitFor("grabación parada al terminar la llamada", () => (state === "stopped" ? true : null), 180_000);
    check(!!stopped, "al terminar la llamada la grabación para sola");
    if (!stopped) await ipc.stopRecording().catch(() => {});
    if (noteId) {
      await sleep(1000);
      await waitFor("enhance terminado", () => !document.querySelector(".enhancing") || null, 120_000);
      await ipc.trashNote(noteId).catch(() => {});
    }
  } finally {
    un.then((f) => f());
  }
}
