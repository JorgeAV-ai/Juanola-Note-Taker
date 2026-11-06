import type { Route } from "../App";

// Estructura del sidebar de Granola: My notes, Team space, carpetas, Customer calls,
// Recipes, Views (People/Companies), Trash, y menú del workspace abajo a la izquierda.
export function Sidebar({ onNavigate }: { onNavigate: (r: Route) => void }) {
  return (
    <aside className="sidebar">
      <div className="sidebar-top" data-tauri-drag-region />
      <button className="sidebar-item" onClick={() => onNavigate({ kind: "home" })}>
        Home
      </button>

      <SpaceHeader name="My notes" />
      <SpaceHeader name="Team space" />

      <div className="sidebar-section">
        <button className="sidebar-item">Customer calls</button>
        <button className="sidebar-item">Recipes</button>
      </div>

      <div className="sidebar-bottom">
        <button className="sidebar-item muted">Views</button>
        <button className="workspace-menu">
          <span className="workspace-avatar" />
          <span>My workspace</span>
        </button>
      </div>
    </aside>
  );
}

function SpaceHeader({ name }: { name: string }) {
  return (
    <div className="space">
      <div className="space-header">
        <span>{name}</span>
        <button className="icon-btn" title="New folder">+</button>
      </div>
    </div>
  );
}
