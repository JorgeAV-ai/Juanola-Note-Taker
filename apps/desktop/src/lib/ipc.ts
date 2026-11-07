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
  source: Citation | null;
}

export interface EnhancedNotes {
  sections: { heading: string; bullets: Bullet[] }[];
}

export interface CalendarEvent {
  id: string;
  title: string;
  start: string;
  end: string;
  attendees: { email: string; name: string | null }[];
  conference_url: string | null;
}

export type RecordingState = "starting" | "recording" | "paused" | "finishing" | "stopped" | "error" | "warning";

export interface RecordingStateEvent {
  note_id: string;
  state: RecordingState;
  message?: string;
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

export const ipc = {
  appInfo: () => invoke<{ version: string; data_dir: string }>("app_info"),
  listNotes: () => invoke<Note[]>("list_notes"),
  createNote: (title?: string) => invoke<Note>("create_note", { title }),
  getNote: (id: string) => invoke<Note | null>("get_note", { id }),
  setNoteTitle: (id: string, title: string) => invoke<void>("set_note_title", { id, title }),
  setNoteRawDoc: (id: string, doc: unknown) => invoke<void>("set_note_raw_doc", { id, doc }),
  listSegments: (noteId: string) => invoke<Segment[]>("list_segments", { noteId }),
  startRecording: (noteId: string, language?: string) => invoke<void>("start_recording", { noteId, language }),
  pauseRecording: () => invoke<void>("pause_recording"),
  resumeRecording: () => invoke<void>("resume_recording"),
  stopRecording: () => invoke<number>("stop_recording"),
  enhanceNote: (id: string) => invoke<EnhancedNotes>("enhance_note", { id }),
  comingUp: () => invoke<CalendarEvent[]>("coming_up"),
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
};
