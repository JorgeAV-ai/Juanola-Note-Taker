// Carpetas del sidebar. Se recargan tras cualquier cambio (crear, renombrar,
// mover una nota, papelera) para que los recuentos estén al día.
import { create } from "zustand";
import { ipc, type Folder } from "../lib/ipc";

interface LibraryState {
  folders: Folder[];
  reload: () => Promise<void>;
}

export const useLibrary = create<LibraryState>((set) => ({
  folders: [],
  reload: async () => {
    try {
      set({ folders: await ipc.listFolders() });
    } catch {
      set({ folders: [] });
    }
  },
}));
