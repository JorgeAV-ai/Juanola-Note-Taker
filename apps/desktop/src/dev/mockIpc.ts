// Backend simulado para abrir el frontend en un navegador normal (`pnpm dev`
// sin Tauri). Solo se carga en desarrollo y fuera de Tauri (ver main.tsx). Las
// llamadas quedan en `window.__ipcCalls` para inspeccionarlas.
import { mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { EnhancedNotes, Folder, ModelStatus, Note, Segment } from "../lib/ipc";

const NOTE_ID = "00000000-0000-4000-8000-000000000001";
const now = new Date().toISOString();

const segments: Segment[] = [
  { id: "s1", note_id: NOTE_ID, channel: "system", start: 2, end: 9, text: "Las ventas de octubre han subido un 12 por ciento.", is_final: true },
  { id: "s2", note_id: NOTE_ID, channel: "mic", start: 10, end: 15, text: "Perfecto, Laura prepara la presentación para el viernes.", is_final: true },
  { id: "s3", note_id: NOTE_ID, channel: "system", start: 16, end: 22, text: "Lanzamos la nueva versión el 20 de noviembre.", is_final: true },
];

const enhanced: EnhancedNotes = {
  sections: [
    {
      heading: "Ventas",
      bullets: [
        { text: "Mis notas: revisar el canal de partners", is_user_text: true, is_action_item: false, source: { kind: "user_note", note_id: NOTE_ID, block_id: "b1" } },
        { text: "Las ventas de octubre subieron un 12 %", is_user_text: false, is_action_item: false, source: { kind: "transcript", note_id: NOTE_ID, start: 2, end: 9 } },
        { text: "Lanzamiento el 20 de noviembre", is_user_text: false, is_action_item: false, source: { kind: "transcript", note_id: NOTE_ID, start: 16, end: 22 } },
      ],
    },
    {
      heading: "Próximos pasos",
      bullets: [
        { text: "Laura: preparar la presentación para el viernes", is_user_text: false, is_action_item: true, done: false, source: { kind: "transcript", note_id: NOTE_ID, start: 10, end: 15 } },
      ],
    },
  ],
};

const notes: Note[] = [
  {
    id: NOTE_ID,
    workspace_id: "00000000-0000-0000-0000-000000000000",
    title: "Reunión de prueba",
    raw_doc: null,
    enhanced_doc: enhanced,
    template_id: null,
    status: "ready",
    calendar_event_id: null,
    created_at: now,
    updated_at: now,
    trashed_at: null,
  },
];

// Modelos: con `?models=missing` en la URL faltan dos, para probar el diálogo.
const modelsMissing = new URLSearchParams(location.search).get("models") === "missing";
const models: ModelStatus[] = [
  { slot: "vad", name: "silero-vad", format: "mlx", repo: "mlx-community/silero-vad", size_mb: 2, dir: "/mock/vad", installed: true, partial_bytes: 0 },
  { slot: "stt_streaming", name: "qwen3-asr-0.6b", format: "mlx", repo: "mlx-community/Qwen3-ASR-0.6B-4bit", size_mb: 710, dir: "/mock/stt", installed: !modelsMissing, partial_bytes: 0 },
  { slot: "llm_main", name: "lfm3-1.7b", format: "gguf", repo: "LiquidAI/LFM3-1.7B-GGUF", size_mb: 1100, dir: "/mock/llm", installed: !modelsMissing, partial_bytes: 0 },
];
let installTimer: ReturnType<typeof setInterval> | null = null;

const folders: Folder[] = [];
const folderOf = new Map<string, string>();
const trashed = new Set<string>();
const countFolders = () => folders.forEach((f) => (f.note_count = [...folderOf].filter(([n, id]) => id === f.id && !trashed.has(n)).length));

declare global {
  interface Window {
    __ipcCalls: { cmd: string; args: unknown }[];
  }
}

export function installMockIpc() {
  window.__ipcCalls = [];
  let recordingNote: string | null = null;
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      if (!cmd.startsWith("plugin:event")) window.__ipcCalls.push({ cmd, args: JSON.parse(JSON.stringify(a)) });
      const note = (id: unknown) => notes.find((n) => n.id === id) ?? null;
      switch (cmd) {
        case "list_notes":
          return notes.filter((n) => !trashed.has(n.id));
        case "trash_note":
          trashed.add(a.id as string);
          countFolders();
          return null;
        case "restore_note":
          trashed.delete(a.id as string);
          countFolders();
          return null;
        case "list_trashed_notes":
          return notes.filter((n) => trashed.has(n.id));
        case "delete_note_forever": {
          const i = notes.findIndex((n) => n.id === a.id && trashed.has(n.id));
          if (i >= 0) notes.splice(i, 1);
          return null;
        }
        case "list_folders":
          countFolders();
          return structuredClone(folders);
        case "create_folder": {
          const f = { id: crypto.randomUUID(), name: (a.name as string).trim(), note_count: 0 };
          folders.push(f);
          return f;
        }
        case "rename_folder":
          folders.find((f) => f.id === a.id)!.name = a.name as string;
          return null;
        case "delete_folder": {
          folders.splice(folders.findIndex((f) => f.id === a.id), 1);
          for (const [n, id] of folderOf) if (id === a.id) folderOf.delete(n);
          return null;
        }
        case "list_folder_notes":
          return notes.filter((n) => folderOf.get(n.id) === a.id && !trashed.has(n.id));
        case "get_note_folder":
          return folderOf.get(a.noteId as string) ?? null;
        case "set_note_folder":
          if (a.folderId) folderOf.set(a.noteId as string, a.folderId as string);
          else folderOf.delete(a.noteId as string);
          return null;
        case "list_audio_devices":
          return [
            { id: "1", name: "MacBook Pro Microphone", is_default: false, is_bluetooth: false },
            { id: "2", name: "JBL Tune 720BT", is_default: true, is_bluetooth: true },
          ];
        case "mic_permission_status":
          return "granted";
        case "reveal_data_dir":
          return null;
        case "create_note": {
          const n: Note = { ...notes[0], id: crypto.randomUUID(), title: "", enhanced_doc: null, status: "draft" };
          notes.unshift(n);
          return n;
        }
        case "get_note":
          return note(a.id);
        case "set_note_title":
          note(a.id)!.title = a.title as string;
          return null;
        case "set_note_raw_doc":
          note(a.id)!.raw_doc = a.doc;
          return null;
        case "set_note_enhanced_doc":
          note(a.id)!.enhanced_doc = a.doc as EnhancedNotes;
          return null;
        case "list_segments":
          return a.noteId === NOTE_ID ? segments : [];
        case "models_status":
          return { models: structuredClone(models), installing: installTimer !== null };
        case "install_models": {
          if (installTimer) return null;
          // Descarga simulada: 40 pasos de 100 ms por modelo que falte.
          const queue = models.filter((m) => !m.installed);
          let step = 0;
          installTimer = setInterval(() => {
            const m = queue[0];
            if (!m) {
              clearInterval(installTimer!);
              installTimer = null;
              emit("models://done", { ok: true });
              return;
            }
            const total = (m.size_mb ?? 1) * 1e6;
            step += 1;
            emit("models://progress", { slot: m.slot, file: "model", done: Math.min(total, (total * step) / 40), total });
            if (step >= 40) {
              m.installed = true;
              queue.shift();
              step = 0;
            }
          }, 100);
          return null;
        }
        case "cancel_model_install":
          if (installTimer) {
            clearInterval(installTimer);
            installTimer = null;
            emit("models://done", { ok: false, error: "descarga cancelada" });
          }
          return null;
        case "start_recording": {
          if (models.some((m) => m.slot !== "llm_main" && !m.installed)) throw "Faltan modelos por descargar.";
          recordingNote = a.noteId as string;
          const id = recordingNote;
          setTimeout(() => emit("recording://state", { note_id: id, state: "starting" }), 10);
          setTimeout(() => emit("recording://state", { note_id: id, state: "recording" }), 300);
          const seg = (sid: string, channel: string, text: string, is_final: boolean, at: number) =>
            setTimeout(() => emit("transcript://segment", { note_id: id, channel, id: sid, start: at, end: at + 2, text, is_final }), at * 100);
          seg("l1", "mic", "Vale.", true, 8);
          seg("l2", "system", "Resumen del standup de esta mañana: vamos bien para la release, pero hay un par de cosas que vigilar.", true, 12);
          seg("l3", "mic", "Primero, el estado.", false, 16);
          setTimeout(
            () =>
              emit("recording://state", {
                note_id: id,
                state: "warning",
                message: "El micrófono solo da silencio. Si no lo tienes silenciado, revisa el permiso de micrófono de Juanola.",
                action: "mic_settings",
              }),
            600,
          );
          return null;
        }
        case "pause_recording":
          emit("recording://state", { note_id: recordingNote, state: "paused" });
          return null;
        case "resume_recording":
          emit("recording://state", { note_id: recordingNote, state: "recording" });
          return null;
        case "stop_recording":
          emit("recording://state", { note_id: recordingNote, state: "stopped" });
          return 3; // frases guardadas: así el mock sigue con el enhance
        case "enhance_note":
          if (models.some((m) => m.slot === "llm_main" && !m.installed)) throw "Faltan modelos por descargar.";
          return new Promise((r) => setTimeout(() => r(note(a.id)?.enhanced_doc ?? enhanced), 2500));
        case "frontend_error":
          console.error(`[frontend_error] ${a.message}`);
          return null;
        case "autotest_log":
          console.info(`[autotest] ${a.line}`);
          return null;
        case "open_privacy_settings":
          return null;
        case "autotest_config":
          return null;
        case "coming_up":
          return [];
        case "app_info":
          return { version: "dev", data_dir: "/mock" };
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}
