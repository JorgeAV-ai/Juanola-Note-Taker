// Puente tipado con los comandos Rust. En fase 1 estos tipos los genera tauri-specta.
import { invoke } from "@tauri-apps/api/core";

export type NoteStatus = "draft" | "transcribing" | "enhancing" | "ready" | "failed";

export interface Note {
  id: string;
  workspace_id: string;
  title: string;
  raw_doc: unknown;
  enhanced_doc: unknown | null;
  template_id: string | null;
  status: NoteStatus;
  calendar_event_id: string | null;
  created_at: string;
  updated_at: string;
  trashed_at: string | null;
}

export interface CalendarEvent {
  id: string;
  title: string;
  start: string;
  end: string;
  attendees: { email: string; name: string | null }[];
  conference_url: string | null;
}

export const ipc = {
  appInfo: () => invoke<{ version: string; data_dir: string }>("app_info"),
  listNotes: () => invoke<Note[]>("list_notes"),
  createNote: (title?: string) => invoke<Note>("create_note", { title }),
  comingUp: () => invoke<CalendarEvent[]>("coming_up"),
};
