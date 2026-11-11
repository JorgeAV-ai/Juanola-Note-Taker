import { create } from "zustand";
import { events, ipc, type ModelProgress, type ModelStatus } from "@/lib/ipc";
import { t } from "@/lib/i18n";
import type { Key } from "@/lib/strings";

// Estado de los modelos locales y de su descarga. Lo comparten el diálogo de
// descarga (que se abre solo si faltan) y Ajustes › Models.
type State = {
  models: ModelStatus[];
  installing: boolean;
  progress: ModelProgress | null;
  error: string | null;
  dialogOpen: boolean;
  loaded: boolean;
  refresh: () => Promise<void>;
  install: () => void;
  cancel: () => void;
  openDialog: (open: boolean) => void;
};

export const missing = (models: ModelStatus[]) => models.filter((m) => !m.installed);

export const useModels = create<State>((set, get) => ({
  models: [],
  installing: false,
  progress: null,
  error: null,
  dialogOpen: false,
  loaded: false,
  refresh: async () => {
    try {
      const s = await ipc.modelsStatus();
      set({ models: s.models, installing: s.installing, loaded: true });
    } catch (e) {
      set({ error: String(e), loaded: true });
    }
  },
  install: () => {
    set({ installing: true, error: null, progress: null });
    ipc.installModels().catch((e) => set({ installing: false, error: String(e) }));
  },
  cancel: () => {
    ipc.cancelModelInstall().catch(() => {});
  },
  openDialog: (open) => {
    set({ dialogOpen: open });
    if (open) get().refresh();
  },
}));

let wired = false;
/** Escucha los eventos de descarga (una vez) y carga el estado inicial. */
export function initModels() {
  if (wired) return;
  wired = true;
  events.modelsProgress((p) => {
    const prev = useModels.getState().progress;
    useModels.setState({ progress: p, installing: true });
    // Ha terminado un modelo y empieza otro: refresca para marcarlo instalado.
    if (prev && prev.slot !== p.slot) useModels.getState().refresh();
  });
  events.modelsDone((d) => {
    // Pausar no es un error: lo bajado se queda en `.part` y continúa después.
    const paused = d.error?.includes("cancelada");
    useModels.setState({ installing: false, progress: null, error: d.ok || paused ? null : (d.error ?? t("models.failed")) });
    useModels.getState().refresh();
  });
  useModels
    .getState()
    .refresh()
    .then(() => {
      // Primera vez sin modelos: se ofrece la descarga nada más abrir.
      if (missing(useModels.getState().models).length) useModels.setState({ dialogOpen: true });
    });
}

export const formatBytes = (n: number) => (n >= 1e9 ? `${(n / 1e9).toFixed(2)} GB` : `${Math.max(1, Math.round(n / 1e6))} MB`);
export const SLOT_LABEL: Record<string, Key> = {
  vad: "models.slot.vad",
  stt_streaming: "models.slot.stt",
  llm_main: "models.slot.llm",
};
