// Puente tipado con los comandos Rust. En fase 1 estos tipos los genera tauri-specta.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type NoteStatus = "draft" | "transcribing" | "enhancing" | "ready" | "failed";
export type Channel = "mic" | "system";

export interface Note {
  id: string;
  workspace_id: string;
  title: string;
  raw_doc: unknown;
  enhanced_doc: EnhancedNotes | null;
  template_id: string | null;
  status: NoteStatus;
  calendar_event_id: string | null;
  created_at: string;
  updated_at: string;
  trashed_at: string | null;
}

export interface Segment {
  id: string;
  note_id: string;
  channel: Channel;
  start: number;
  end: number;
  text: string;
  is_final: boolean;
}

export type Citation =
  | { kind: "transcript"; note_id: string; start: number; end: number }
  | { kind: "user_note"; note_id: string; block_id: string };

export interface Bullet {
  text: string;
  is_user_text: boolean;
  is_action_item: boolean;
  /** Tarea marcada como hecha. */
  done?: boolean;
  source: Citation | null;
}

export interface EnhancedNotes {
  sections: { heading: string; bullets: Bullet[] }[];
  /** El usuario las ha editado: regenerarlas perdería esos cambios. */
  edited?: boolean;
}

export type SectionKind = "topics" | "tasks";

export interface TemplateSection {
  heading: string;
  hint: string;
  kind: SectionKind;
}

/** Plantilla del resumen. Las integradas traen los títulos en el idioma pedido. */
export interface Template {
  id: string;
  name: string;
  purpose: string;
  style: string;
  structure: TemplateSection[];
  is_builtin: boolean;
  shared_with_workspace: boolean;
}

/** Plantilla para `enhanceNote`: «auto», el id de una plantilla o nada (la de la nota). */
export type TemplateChoice = "auto" | string;

/** Evento `note://status`. `notice: "template_fallback"`: la plantilla no encajó y se usó Auto. */
export interface NoteStatusEvent {
  note_id: string;
  status: NoteStatus;
  message?: string;
  notice?: "template_fallback";
}

export interface Attendee {
  email: string;
  name: string | null;
  declined: boolean;
  is_self: boolean;
}

export interface CalendarEvent {
  id: string;
  provider: string;
  calendar_id: string;
  title: string;
  start: string;
  end: string;
  attendees: Attendee[];
  conference_url: string | null;
  recurring_event_id: string | null;
  description: string | null;
}

export type CalendarAccess = "granted" | "denied" | "not_determined" | "write_only" | "unsupported";

export interface DetectionSettings {
  /** Avisar cuando otra app abre el micrófono. */
  meetings: boolean;
  /** Avisar 1 minuto antes de los eventos del calendario. */
  calendar_reminders: boolean;
  /** Parar la grabación tras 15 minutos sin voz. */
  auto_stop: boolean;
}

export interface MeetingEnded {
  note_id: string;
  app: DetectedApp;
}

export interface DetectedApp {
  bundle_id: string;
  name: string;
  kind: string;
}

/** Aviso de reunión (ventana `notification`, ver detect.rs). */
export interface MeetingNotice {
  kind: "adhoc" | "calendar";
  title: string;
  body: string;
  app: DetectedApp | null;
  event: CalendarEvent | null;
}

/** Sesión en curso para la barra de menús y el indicador flotante. */
export type IndicatorSnapshot = { note_id: string; phase: "starting" | "recording" | "paused"; secs: number };
export type TrayLabels = {
  open: string;
  new_note: string;
  go_to_note: string;
  pause: string;
  resume: string;
  stop: string;
  recording: string;
  paused: string;
  quit: string;
};

export type ExportLabels = { untitled: string; summary: string; my_notes: string; transcript: string; me: string; them: string };

export type RecordingState = "starting" | "recording" | "paused" | "finishing" | "stopped" | "error" | "warning";

export type SettingsAction = "mic_settings" | "audio_capture_settings" | "calendar_settings";

export interface RecordingStateEvent {
  note_id: string;
  state: RecordingState;
  message?: string;
  /** Ajuste del sistema que arregla el problema (se ofrece un botón). */
  action?: SettingsAction;
}

export interface LiveSegment {
  note_id: string;
  channel: Channel;
  id: string;
  start: number;
  end: number;
  text: string;
  is_final: boolean;
}

export interface Folder {
  id: string;
  name: string;
  note_count: number;
}

export interface AudioDevice {
  id: string;
  name: string;
  is_default: boolean;
  is_bluetooth: boolean;
}

export type Permission = "granted" | "denied" | "not_determined" | "unknown";

export type ModelStatus = {
  slot: string;
  name: string;
  format: string;
  repo: string;
  size_mb: number | null;
  dir: string;
  installed: boolean;
  partial_bytes: number;
};
export type ModelsState = { models: ModelStatus[]; installing: boolean };
export type ModelProgress = { slot: string; file: string; done: number; total: number };
/** Error de start_recording / enhance_note cuando faltan modelos (models.rs). */
export const MODELS_MISSING = "Faltan modelos por descargar.";

export const ipc = {
  appInfo: () => invoke<{ version: string; data_dir: string }>("app_info"),
  listNotes: () => invoke<Note[]>("list_notes"),
  createNote: (title?: string) => invoke<Note>("create_note", { title }),
  getNote: (id: string) => invoke<Note | null>("get_note", { id }),
  setNoteTitle: (id: string, title: string) => invoke<void>("set_note_title", { id, title }),
  setNoteRawDoc: (id: string, doc: unknown) => invoke<void>("set_note_raw_doc", { id, doc }),
  setNoteEnhancedDoc: (id: string, doc: EnhancedNotes) => invoke<void>("set_note_enhanced_doc", { id, doc }),
  listSegments: (noteId: string) => invoke<Segment[]>("list_segments", { noteId }),
  startRecording: (noteId: string, language?: string) => invoke<void>("start_recording", { noteId, language }),
  pauseRecording: () => invoke<void>("pause_recording"),
  resumeRecording: () => invoke<void>("resume_recording"),
  stopRecording: () => invoke<number>("stop_recording"),
  recordingState: () => invoke<{ note_id: string | null; stopping_note: string | null; paused: boolean; stopping: boolean }>("recording_state"),
  enhanceNote: (id: string, template?: TemplateChoice) => invoke<EnhancedNotes>("enhance_note", { id, template }),
  /** `language`: «es»/«en» (interfaz) o el nombre del idioma del resumen («inglés»). */
  listTemplates: (language?: string) => invoke<Template[]>("list_templates", { language }),
  saveTemplate: (template: Template) => invoke<Template>("save_template", { template }),
  deleteTemplate: (id: string) => invoke<void>("delete_template", { id }),
  /** Fija la plantilla sin generar el resumen; `null` es Auto. */
  setNoteTemplate: (id: string, templateId: string | null) => invoke<void>("set_note_template", { id, templateId }),
  openPrivacySettings: (kind: SettingsAction) => invoke<void>("open_privacy_settings", { kind }),
  comingUp: () => invoke<CalendarEvent[]>("coming_up"),
  calendarAccess: () => invoke<CalendarAccess>("calendar_access"),
  requestCalendarAccess: () => invoke<CalendarAccess>("request_calendar_access"),
  openEventNote: (event: CalendarEvent) => invoke<Note>("open_event_note", { event }),
  noteEvent: (noteId: string) => invoke<CalendarEvent | null>("note_event", { noteId }),
  sttGlossary: () => invoke<string>("stt_glossary"),
  setSttGlossary: (glossary: string) => invoke<void>("set_stt_glossary", { glossary }),
  detectionSettings: () => invoke<DetectionSettings>("detection_settings"),
  setDetectionSettings: (settings: DetectionSettings) => invoke<void>("set_detection_settings", { settings }),
  notificationPayload: () => invoke<MeetingNotice | null>("notification_payload"),
  notificationTakeNotes: () => invoke<void>("notification_take_notes"),
  notificationDismiss: () => invoke<void>("notification_dismiss"),
  simulateDetection: (kind: "adhoc" | "calendar") => invoke<void>("simulate_detection", { kind }),
  indicatorState: () => invoke<IndicatorSnapshot | null>("indicator_state"),
  indicatorOpen: () => invoke<void>("indicator_open"),
  indicatorPause: (pause: boolean) => invoke<void>("indicator_pause", { pause }),
  indicatorStop: () => invoke<void>("indicator_stop"),
  setTrayLabels: (labels: TrayLabels) => invoke<void>("set_tray_labels", { labels }),
  noteSummaryMarkdown: (id: string) => invoke<string>("note_summary_markdown", { id }),
  /** Abre el diálogo de guardar y escribe la nota; devuelve la ruta o `null` si se canceló. */
  exportNote: (id: string, format: "markdown" | "html", labels: ExportLabels) => invoke<string | null>("export_note", { id, format, labels }),
  trashNote: (id: string) => invoke<void>("trash_note", { id }),
  restoreNote: (id: string) => invoke<void>("restore_note", { id }),
  listTrashedNotes: () => invoke<Note[]>("list_trashed_notes"),
  deleteNoteForever: (id: string) => invoke<void>("delete_note_forever", { id }),
  listFolders: () => invoke<Folder[]>("list_folders"),
  createFolder: (name: string) => invoke<Folder>("create_folder", { name }),
  renameFolder: (id: string, name: string) => invoke<void>("rename_folder", { id, name }),
  deleteFolder: (id: string) => invoke<void>("delete_folder", { id }),
  listFolderNotes: (id: string) => invoke<Note[]>("list_folder_notes", { id }),
  getNoteFolder: (noteId: string) => invoke<string | null>("get_note_folder", { noteId }),
  setNoteFolder: (noteId: string, folderId: string | null) => invoke<void>("set_note_folder", { noteId, folderId }),
  listAudioDevices: () => invoke<AudioDevice[]>("list_audio_devices"),
  micPermissionStatus: () => invoke<Permission>("mic_permission_status"),
  revealDataDir: () => invoke<void>("reveal_data_dir"),
  modelsStatus: () => invoke<ModelsState>("models_status"),
  installModels: () => invoke<void>("install_models"),
  cancelModelInstall: () => invoke<void>("cancel_model_install"),
};

export const events = {
  recordingState: (cb: (e: RecordingStateEvent) => void): Promise<UnlistenFn> =>
    listen<RecordingStateEvent>("recording://state", (e) => cb(e.payload)),
  levels: (cb: (e: { note_id: string; mic: number; system: number }) => void): Promise<UnlistenFn> =>
    listen<{ note_id: string; mic: number; system: number }>("recording://levels", (e) => cb(e.payload)),
  segment: (cb: (e: LiveSegment) => void): Promise<UnlistenFn> =>
    listen<LiveSegment>("transcript://segment", (e) => cb(e.payload)),
  noteStatus: (cb: (e: NoteStatusEvent) => void): Promise<UnlistenFn> => listen<NoteStatusEvent>("note://status", (e) => cb(e.payload)),
  calendarChanged: (cb: () => void): Promise<UnlistenFn> => listen("calendar://changed", () => cb()),
  takeNotes: (cb: (e: { note_id: string }) => void): Promise<UnlistenFn> =>
    listen<{ note_id: string }>("detect://take-notes", (e) => cb(e.payload)),
  /** La llamada que se grababa terminó: hay que parar la grabación de `note_id`. */
  meetingEnded: (cb: (e: MeetingEnded) => void): Promise<UnlistenFn> => listen<MeetingEnded>("detect://meeting-ended", (e) => cb(e.payload)),
  /** El indicador o la barra de menús piden parar la grabación de `note_id`. */
  stopRequested: (cb: (e: { note_id: string }) => void): Promise<UnlistenFn> =>
    listen<{ note_id: string }>("recording://stop-requested", (e) => cb(e.payload)),
  /** El indicador o la barra de menús piden abrir la nota que se graba. */
  indicatorOpenNote: (cb: (e: { note_id: string }) => void): Promise<UnlistenFn> =>
    listen<{ note_id: string }>("indicator://open-note", (e) => cb(e.payload)),
  indicatorUpdate: (cb: (e: IndicatorSnapshot | null) => void): Promise<UnlistenFn> =>
    listen<IndicatorSnapshot | null>("indicator://update", (e) => cb(e.payload)),
  /** «Grabar una nota nueva» en la barra de menús. */
  trayNewNote: (cb: () => void): Promise<UnlistenFn> => listen("tray://new-note", () => cb()),
  notificationUpdate: (cb: (e: MeetingNotice) => void): Promise<UnlistenFn> =>
    listen<MeetingNotice>("notification://update", (e) => cb(e.payload)),
  modelsProgress: (cb: (e: ModelProgress) => void): Promise<UnlistenFn> => listen<ModelProgress>("models://progress", (e) => cb(e.payload)),
  modelsDone: (cb: (e: { ok: boolean; error?: string }) => void): Promise<UnlistenFn> =>
    listen<{ ok: boolean; error?: string }>("models://done", (e) => cb(e.payload)),
};
