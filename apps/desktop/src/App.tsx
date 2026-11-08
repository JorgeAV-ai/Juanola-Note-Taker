import { useEffect, useState } from "react";
import { Sidebar } from "@/components/Sidebar";
import { Home } from "@/components/Home";
import { NoteView } from "@/components/NoteView";
import { FolderView } from "@/components/FolderView";
import { TrashView } from "@/components/TrashView";
import { SettingsView, type SettingsSection } from "@/components/SettingsView";
import { SoonView, type SoonKind } from "@/components/SoonView";
import { ChatView } from "@/components/ChatView";
import { SearchDialog } from "@/components/SearchDialog";
import { ModelsDialog } from "@/components/ModelsDialog";
import { initModels } from "@/state/models";
import { TopBar } from "@/components/TopBar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useLibrary } from "@/state/library";
import { ipc } from "@/lib/ipc";

export type Route =
  | { kind: "home" }
  | { kind: "space" }
  | { kind: "note"; id: string }
  | { kind: "folder"; id: string }
  | { kind: "trash" }
  | { kind: "chat"; q?: string }
  | { kind: "settings"; section?: SettingsSection }
  | { kind: "soon"; what: SoonKind };

export default function App() {
  const [route, setRoute] = useState<Route>({ kind: "home" });
  const [sidebar, setSidebar] = useState(true);
  const [search, setSearch] = useState(false);
  const reload = useLibrary((s) => s.reload);

  useEffect(() => {
    reload();
    initModels();
  }, [reload]);

  // Atajos de Granola: ⌘K búsqueda, ⌘, ajustes, ⌘N nota nueva.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      if (e.key === "k") {
        e.preventDefault();
        setSearch(true);
      } else if (e.key === ",") {
        e.preventDefault();
        setRoute({ kind: "settings" });
      } else if (e.key === "n") {
        e.preventDefault();
        newNote();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const openNote = (id: string) => setRoute({ kind: "note", id });
  const newNote = async (folderId?: string) => {
    const n = await ipc.createNote();
    if (folderId) {
      await ipc.setNoteFolder(n.id, folderId);
      await reload();
    }
    openNote(n.id);
  };

  if (route.kind === "settings") {
    return (
      <TooltipProvider>
        <SettingsView section={route.section ?? "preferences"} onNavigate={setRoute} />
      </TooltipProvider>
    );
  }

  let view;
  switch (route.kind) {
    case "home":
      view = <Home onOpenNote={openNote} onNavigate={setRoute} />;
      break;
    case "space":
      view = <Home onOpenNote={openNote} onNavigate={setRoute} space />;
      break;
    case "note":
      view = <NoteView key={route.id} id={route.id} onTrashed={() => setRoute({ kind: "home" })} />;
      break;
    case "folder":
      view = <FolderView key={route.id} id={route.id} onOpenNote={openNote} onNewNote={() => newNote(route.id)} onDeleted={() => setRoute({ kind: "home" })} />;
      break;
    case "trash":
      view = <TrashView />;
      break;
    case "chat":
      view = <ChatView initial={route.q} onOpenNote={openNote} />;
      break;
    case "soon":
      view = <SoonView what={route.what} />;
      break;
  }
  return (
    <TooltipProvider>
      <div className="flex h-full bg-canvas">
        {/* En una nota, como en Granola, la nota ocupa toda la ventana. */}
        {sidebar && route.kind !== "note" && <Sidebar route={route} onNavigate={setRoute} onSearch={() => setSearch(true)} onCollapse={() => setSidebar(false)} />}
        <main className="relative flex min-w-0 flex-1 flex-col">
          <TopBar
            sidebar={sidebar && route.kind !== "note"}
            inNote={route.kind === "note"}
            onExpand={() => setSidebar(true)}
            onSearch={() => setSearch(true)}
            onHome={() => setRoute({ kind: "home" })}
            onNewNote={route.kind === "note" ? undefined : () => newNote(route.kind === "folder" ? route.id : undefined)}
          />
          <div className="main-scroll min-h-0 flex-1 overflow-auto">{view}</div>
        </main>
      </div>
      <SearchDialog open={search} onOpenChange={setSearch} onOpenNote={openNote} />
      <ModelsDialog />
    </TooltipProvider>
  );
}
