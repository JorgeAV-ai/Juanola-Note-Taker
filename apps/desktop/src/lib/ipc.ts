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
  enhanceNote: (id: string) => invoke<EnhancedNotes>("enhance_note", { id }),
  openPrivacySettings: (kind: SettingsAction) => invoke<void>("open_privacy_settings", { kind }),
  comingUp: () => invoke<CalendarEvent[]>("coming_up"),
  calendarAccess: () => invoke<CalendarAccess>("calendar_access"),
  requestCalendarAccess: () => invoke<CalendarAccess>("request_calendar_access"),
  openEventNote: (event: CalendarEvent) => invoke<Note>("open_event_note", { event }),
  noteEvent: (noteId: string) => invoke<CalendarEvent | null>("note_event", { noteId }),
  detectionSettings: () => invoke<DetectionSettings>("detection_settings"),
  setDetectionSettings: (settings: DetectionSettings) => invoke<void>("set_detection_settings", { settings }),
  notificationPayload: () => invoke<MeetingNotice | null>("notification_payload"),
  notificationTakeNotes: () => invoke<void>("notification_take_notes"),
  notificationDismiss: () => invoke<void>("notification_dismiss"),
  simulateDetection: (kind: "adhoc" | "calendar") => invoke<void>("simulate_detection", { kind }),
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
  levels: (cb: (e: { mic: number; system: number }) => void): Promise<UnlistenFn> =>
    listen<{ mic: number; system: number }>("recording://levels", (e) => cb(e.payload)),
  segment: (cb: (e: LiveSegment) => void): Promise<UnlistenFn> =>
    listen<LiveSegment>("transcript://segment", (e) => cb(e.payload)),
  noteStatus: (cb: (e: { note_id: string; status: NoteStatus; message?: string }) => void): Promise<UnlistenFn> =>
    listen<{ note_id: string; status: NoteStatus; message?: string }>("note://status", (e) => cb(e.payload)),
  calendarChanged: (cb: () => void): Promise<UnlistenFn> => listen("calendar://changed", () => cb()),
  takeNotes: (cb: (e: { note_id: string }) => void): Promise<UnlistenFn> =>
    listen<{ note_id: string }>("detect://take-notes", (e) => cb(e.payload)),
  meetingEnded: (cb: (e: DetectedApp) => void): Promise<UnlistenFn> => listen<DetectedApp>("detect://meeting-ended", (e) => cb(e.payload)),
  notificationUpdate: (cb: (e: MeetingNotice) => void): Promise<UnlistenFn> =>
    listen<MeetingNotice>("notification://update", (e) => cb(e.payload)),
  modelsProgress: (cb: (e: ModelProgress) => void): Promise<UnlistenFn> => listen<ModelProgress>("models://progress", (e) => cb(e.payload)),
  modelsDone: (cb: (e: { ok: boolean; error?: string }) => void): Promise<UnlistenFn> =>
    listen<{ ok: boolean; error?: string }>("models://done", (e) => cb(e.payload)),
};
