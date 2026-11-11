import { useRef, useState, type ReactNode } from "react";
import { FileText, Folder, FolderPlus, House, MessageCircle, PanelLeft, Search, Settings, Trash2 } from "lucide-react";
import type { Route } from "@/App";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useLibrary } from "@/state/library";
import { useT } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import { Tip } from "@/components/ui/tooltip";

const same = (a: Route, b: Route) => JSON.stringify(a) === JSON.stringify(b);

// Barra lateral: buscar, Inicio / Mis notas / Chat, las carpetas y, abajo,
// Papelera y Ajustes. Solo lo que existe en una app local sin cuentas.
export function Sidebar({
  route,
  onNavigate,
  onSearch,
  onCollapse,
}: {
  route: Route;
  onNavigate: (r: Route) => void;
  onSearch: () => void;
  onCollapse: () => void;
}) {
  const t = useT();
  const folders = useLibrary((s) => s.folders);
  const reload = useLibrary((s) => s.reload);
  const [naming, setNaming] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Enter y blur disparan los dos: el segundo no debe crear otra carpeta.
  const creating = useRef(false);
  const createFolder = async (name: string) => {
    setNaming(false);
    const trimmed = name.trim();
    if (!trimmed || creating.current) return;
    creating.current = true;
    try {
      const f = await ipc.createFolder(trimmed);
      await reload();
      onNavigate({ kind: "folder", id: f.id });
    } catch (e) {
      setError(String(e));
    } finally {
      creating.current = false;
    }
  };

  return (
    <aside className="sidebar flex w-[248px] shrink-0 flex-col border-r border-line bg-sidebar px-2.5 pb-2.5">
      {/* Fila de los semáforos de macOS */}
      <div className="flex h-12 shrink-0 items-center justify-end gap-0.5" data-tauri-drag-region>
        <Tip label={t("nav.hideSidebar")} side="bottom">
          <Button variant="ghost" size="iconSm" onClick={onCollapse} aria-label={t("nav.hideSidebar")}>
            <PanelLeft />
          </Button>
        </Tip>
      </div>

      <button
        onClick={onSearch}
        className="mb-3 flex h-9 items-center gap-2 rounded-full border border-line bg-canvas/60 px-3 text-[14px] text-ink-2 transition-colors hover:bg-hover"
      >
        <Search className="size-4" />
        <span className="flex-1 text-left">{t("common.search")}</span>
        <kbd className="font-sans text-[12px] text-muted">⌘K</kbd>
      </button>

      <NavItem icon={<House />} label={t("common.home")} active={same(route, { kind: "home" })} onClick={() => onNavigate({ kind: "home" })} />
      <NavItem icon={<FileText />} label={t("nav.myNotes")} active={same(route, { kind: "space" })} onClick={() => onNavigate({ kind: "space" })} />
      <NavItem icon={<MessageCircle />} label={t("nav.chat")} active={route.kind === "chat"} onClick={() => onNavigate({ kind: "chat" })} />

      <div className="mb-1 mt-6 px-2.5 text-[13px] text-muted">{t("nav.folders")}</div>
      <div className="min-h-0 flex-1 overflow-auto">
        {folders.map((f) => (
          <NavItem
            key={f.id}
            className="folder"
            data-folder-id={f.id}
            icon={<Folder />}
            label={f.name}
            badge={f.note_count > 0 ? String(f.note_count) : undefined}
            active={same(route, { kind: "folder", id: f.id })}
            onClick={() => onNavigate({ kind: "folder", id: f.id })}
          />
        ))}
        {naming ? (
          <input
            className="folder-input my-0.5 h-8 w-full rounded-lg border border-line bg-surface px-2.5 text-[14px] outline-none focus:border-olive"
            autoFocus
            placeholder={t("nav.folderName")}
            onKeyDown={(e) => {
              if (e.key === "Enter") createFolder(e.currentTarget.value);
              if (e.key === "Escape") setNaming(false);
            }}
            onBlur={(e) => createFolder(e.currentTarget.value)}
          />
        ) : (
          <NavItem className="text-muted" icon={<FolderPlus />} label={t("nav.addFolder")} title={t("nav.newFolder")} onClick={() => setNaming(true)} />
        )}
        {error && <p className="ml-2.5 mt-1 text-[12px] text-danger">{error}</p>}
      </div>

      <div className="mt-2 border-t border-line pt-2">
        <NavItem className="nav-trash" icon={<Trash2 />} label={t("nav.trash")} active={route.kind === "trash"} onClick={() => onNavigate({ kind: "trash" })} />
        <NavItem
          className="nav-settings"
          icon={<Settings />}
          label={t("nav.settings")}
          badge="⌘,"
          active={route.kind === "settings"}
          onClick={() => onNavigate({ kind: "settings" })}
        />
      </div>
    </aside>
  );
}

function NavItem({
  icon,
  label,
  badge,
  active,
  className,
  ...rest
}: {
  icon: ReactNode;
  label: string;
  badge?: string;
  active?: boolean;
  className?: string;
} & React.ButtonHTMLAttributes<HTMLButtonElement> & { "data-folder-id"?: string }) {
  return (
    <button
      className={cn(
        "sidebar-item flex h-9 w-full items-center gap-2.5 rounded-xl px-2.5 text-left text-[14.5px] text-ink transition-colors hover:bg-hover [&_svg]:size-[17px] [&_svg]:shrink-0 [&_svg]:text-ink-2",
        active && "active bg-active",
        className,
      )}
      aria-label={label}
      {...rest}
    >
      {icon}
      <span className="flex-1 truncate">{label}</span>
      {badge && <span className="count text-[12px] text-muted">{badge}</span>}
    </button>
  );
}
