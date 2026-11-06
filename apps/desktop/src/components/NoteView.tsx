import { NoteEditor } from "./NoteEditor";
import { RecordingPill } from "./RecordingPill";

// Vista de nota: título inline, editor en blanco (My notes / Enhanced), píldora de grabación.
export function NoteView({ id }: { id: string }) {
  return (
    <div className="note" data-note-id={id}>
      <input className="note-title" placeholder="Untitled" />
      <div className="note-tabs">
        <button className="tab active">My notes</button>
        <button className="tab">Enhanced</button>
        <span className="spacer" />
        <button className="icon-btn" title="Change template">✨</button>
      </div>
      <NoteEditor />
      <RecordingPill />
    </div>
  );
}
