import { ChevronLeft, House, PanelLeft, Plus, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tip } from "@/components/ui/tooltip";

// Franja superior del área principal: arrastra la ventana, «+ New note» a la
// derecha y, con el sidebar plegado, sus iconos a la izquierda.
export function TopBar({
  sidebar,
  inNote,
  onExpand,
  onSearch,
  onHome,
  onNewNote,
}: {
  sidebar: boolean;
  inNote?: boolean;
  onExpand: () => void;
  onSearch: () => void;
  onHome: () => void;
  onNewNote?: () => void;
}) {
  return (
    <div className="flex h-12 shrink-0 items-center gap-1 px-3" data-tauri-drag-region>
      {inNote ? (
        <div className="pl-[76px]">
          <button
            className="back-home flex h-8 items-center gap-0.5 rounded-full border border-line px-2.5 text-ink-2 transition-colors hover:bg-hover"
            aria-label="Volver a Home"
            title="Volver a Home"
            onClick={onHome}
          >
            <ChevronLeft className="size-4" />
            <House className="size-4" />
          </button>
        </div>
      ) : !sidebar && (
        <div className="flex items-center gap-1 pl-[68px]">
          <Tip label="Mostrar sidebar" side="bottom">
            <Button variant="ghost" size="iconSm" onClick={onExpand} aria-label="Mostrar sidebar">
              <PanelLeft />
            </Button>
          </Tip>
          <Tip label="Buscar ⌘K" side="bottom">
            <Button variant="ghost" size="iconSm" onClick={onSearch} aria-label="Buscar">
              <Search />
            </Button>
          </Tip>
          <Tip label="Home" side="bottom">
            <Button variant="ghost" size="iconSm" onClick={onHome} aria-label="Home">
              <House />
            </Button>
          </Tip>
        </div>
      )}
      <div className="flex-1" data-tauri-drag-region />
      {onNewNote && (
        <Button onClick={onNewNote} className="new-note">
          <Plus /> New note
        </Button>
      )}
    </div>
  );
}
