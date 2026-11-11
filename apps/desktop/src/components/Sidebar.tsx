import { useState, type ReactNode } from "react";
import {
  Building2,
  ChevronsUpDown,
  Folder,
  FolderPlus,
  House,
  Lock,
  MessageCircle,
  PanelLeft,
  Search,
  Settings,
  SquareSlash,
  Trash2,
  UserRound,
  UsersRound,
} from "lucide-react";
import type { Route } from "@/App";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useLibrary } from "@/state/library";
import { useT } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import { Tip } from "@/components/ui/tooltip";
import { Logo } from "./Logo";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

const same = (a: Route, b: Route) => JSON.stringify(a) === JSON.stringify(b);

// Sidebar de Granola: buscar, Home / Shared with me / Chat, Spaces (My notes y
// sus carpetas), iconos de recetas/personas/empresas y el selector del
// workspace con Ajustes y Papelera.
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

  const createFolder = async (name: string) => {
    setNaming(false);
    if (!name.trim()) return;
    try {
      const f = await ipc.createFolder(name);
      await reload();
      onNavigate({ kind: "folder", id: f.id });
    } catch (e) {
      setError(String(e));
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
      <NavItem icon={<UsersRound />} label={t("nav.shared")} active={same(route, { kind: "soon", what: "shared" })} onClick={() => onNavigate({ kind: "soon", what: "shared" })} />
      <NavItem icon={<MessageCircle />} label={t("nav.chat")} active={route.kind === "chat"} onClick={() => onNavigate({ kind: "chat" })} />

      <div className="mb-1 mt-6 px-2.5 text-[13px] text-muted">{t("nav.spaces")}</div>
      <NavItem
        icon={
          <span className="flex size-[22px] items-center justify-center rounded-md border border-line bg-hover">
            <Lock className="!size-3" />
          </span>
        }
        label={t("nav.myNotes")}
        active={same(route, { kind: "space" })}
        onClick={() => onNavigate({ kind: "space" })}
      />
      <div className="min-h-0 flex-1 overflow-auto">
        {folders.map((f) => (
          <NavItem
            key={f.id}
            className="folder pl-7"
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
            className="folder-input my-0.5 ml-7 h-8 w-[calc(100%-28px)] rounded-lg border border-line bg-surface px-2.5 text-[14px] outline-none focus:border-olive"
            autoFocus
            placeholder={t("nav.folderName")}
            onKeyDown={(e) => {
              if (e.key === "Enter") createFolder(e.currentTarget.value);
              if (e.key === "Escape") setNaming(false);
            }}
            onBlur={(e) => createFolder(e.currentTarget.value)}
          />
        ) : (
          <NavItem className="pl-7 text-muted" icon={<FolderPlus />} label={t("nav.addFolder")} title={t("nav.newFolder")} onClick={() => setNaming(true)} />
        )}
        {error && <p className="ml-7 mt-1 text-[12px] text-danger">{error}</p>}
      </div>

      <div className="mt-2 flex items-center gap-0.5 px-1 pb-2">
        <Tip label={t("nav.recipes")}>
          <Button variant="ghost" size="iconSm" aria-label={t("nav.recipes")} onClick={() => onNavigate({ kind: "soon", what: "recipes" })}>
            <SquareSlash />
          </Button>
        </Tip>
        <Tip label={t("nav.people")}>
          <Button variant="ghost" size="iconSm" aria-label={t("nav.people")} onClick={() => onNavigate({ kind: "soon", what: "people" })}>
            <UserRound />
          </Button>
        </Tip>
        <Tip label={t("nav.companies")}>
          <Button variant="ghost" size="iconSm" aria-label={t("nav.companies")} onClick={() => onNavigate({ kind: "soon", what: "companies" })}>
            <Building2 />
          </Button>
        </Tip>
      </div>

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button className="workspace-menu flex h-10 items-center gap-2.5 rounded-xl px-2 text-[14.5px] transition-colors hover:bg-hover data-[state=open]:bg-hover" aria-label={t("nav.workspace")}>
            <Avatar />
            <span className="flex-1 truncate text-left">{t("nav.myWorkspace")}</span>
            <ChevronsUpDown className="size-4 text-muted" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent side="top" align="start" className="w-[260px]">
          <div className="flex items-center gap-3 px-2.5 py-2">
            <Avatar large />
            <div>
              <div className="text-[14.5px]">{t("nav.myWorkspace")}</div>
              <div className="text-[12.5px] text-muted">{t("nav.members")}</div>
            </div>
          </div>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={() => onNavigate({ kind: "trash" })}>
            <Trash2 /> {t("nav.trash")}
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => onNavigate({ kind: "settings" })}>
            <Settings /> {t("nav.settings")}
            <DropdownMenuShortcut>⌘,</DropdownMenuShortcut>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </aside>
  );
}

function Avatar({ large }: { large?: boolean }) {
  return (
    <span className={cn("flex shrink-0 items-center justify-center rounded-md border border-line bg-[#f6f3ec] dark:border-transparent", large ? "size-9" : "size-6")}>
      <Logo className={large ? "h-6" : "h-4"} />
    </span>
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
