// Transcript en vivo de la nota abierta: parciales y finales por id (un
// parcial se sustituye por su final), más los segmentos ya guardados.
import { create } from "zustand";
import { ipc, type Channel, type LiveSegment } from "../lib/ipc";

export interface Line {
  id: string;
  channel: Channel;
  start: number;
  end: number;
  text: string;
  final: boolean;
}

interface TranscriptState {
  noteId: string | null;
  lines: Line[];
  load: (noteId: string) => Promise<void>;
  apply: (s: LiveSegment) => void;
}

export const useTranscript = create<TranscriptState>((set, get) => ({
  noteId: null,
  lines: [],
  load: async (noteId) => {
    set({ noteId, lines: [] });
    const saved = await ipc.listSegments(noteId);
    if (get().noteId !== noteId) return;
    // Lo que haya llegado en vivo mientras tanto, sin repetir lo guardado (dos
    // `load` seguidos de la misma nota, p. ej. en StrictMode, se solapan). Los
    // ids no sirven para deduplicar: el vivo usa «canal-sesión-id del worker»
    // y lo guardado, un uuid nuevo; se compara por contenido.
    const savedKeys = new Set(saved.map((s) => `${s.channel}:${s.start}:${s.text}`));
    const live = get().lines.filter((l) => !savedKeys.has(`${l.channel}:${l.start}:${l.text}`));
    set({
      lines: [
        ...saved.map((s) => ({ id: s.id, channel: s.channel, start: s.start, end: s.end, text: s.text, final: true })),
        ...live,
      ],
    });
  },
  apply: (s) => {
    if (s.note_id !== get().noteId) return;
    set((st) => {
      const lines = st.lines.filter((l) => l.id !== s.id);
      if (s.text.trim() || !s.is_final) {
        lines.push({ id: s.id, channel: s.channel, start: s.start, end: s.end, text: s.text, final: s.is_final });
      }
      lines.sort((a, b) => a.start - b.start);
      return { lines };
    });
  },
}));

export function excerpt(lines: Line[], start: number, end: number): Line[] {
  return lines.filter((l) => l.end >= start - 0.01 && l.start <= end + 0.01 && l.final);
}
