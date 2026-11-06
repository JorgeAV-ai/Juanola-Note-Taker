import { useState } from "react";

// Píldora abajo-centro con barras animadas, Stop/Resume y acceso al drawer de transcript.
export function RecordingPill() {
  const [recording, setRecording] = useState(false);
  const [open, setOpen] = useState(false);
  return (
    <>
      {open && (
        <div className="transcript-drawer">
          <div className="drawer-head">
            <span>Transcript</span>
            <button className="icon-btn" title="Copy all">⧉</button>
          </div>
          <div className="bubbles">
            <div className="bubble them">Them · (audio del sistema)</div>
            <div className="bubble me">Me · (micrófono)</div>
          </div>
        </div>
      )}
      <div className="pill">
        <button className="icon-btn" title="Transcript" onClick={() => setOpen((o) => !o)}>
          ≋
        </button>
        <div className={`bars ${recording ? "live" : ""}`}>
          <i /><i /><i /><i /><i />
        </div>
        <button className="pill-btn" onClick={() => setRecording((r) => !r)}>
          {recording ? "Stop" : "Start"}
        </button>
      </div>
    </>
  );
}
