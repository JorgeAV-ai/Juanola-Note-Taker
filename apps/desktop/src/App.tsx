import { useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { Home } from "./components/Home";
import { NoteView } from "./components/NoteView";
import { AskAnythingBar } from "./components/AskAnythingBar";

export type Route = { kind: "home" } | { kind: "note"; id: string };

export default function App() {
  const [route, setRoute] = useState<Route>({ kind: "home" });
  return (
    <div className="shell">
      <Sidebar onNavigate={setRoute} />
      <main className="main">
        <div className="main-scroll">
          {route.kind === "home" ? (
            <Home onOpenNote={(id) => setRoute({ kind: "note", id })} />
          ) : (
            <NoteView id={route.id} />
          )}
        </div>
        <AskAnythingBar scope={route.kind === "home" ? "all" : "note"} />
      </main>
    </div>
  );
}
