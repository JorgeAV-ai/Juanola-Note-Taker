// Barra de Granola Chat fija abajo. `/` abre el menú de recipes (fase 3).
export function AskAnythingBar({ scope }: { scope: "all" | "note" }) {
  return (
    <div className="ask-bar">
      <input
        className="ask-input"
        placeholder={scope === "all" ? "Ask anything about your meetings…" : "Ask anything…"}
      />
    </div>
  );
}
