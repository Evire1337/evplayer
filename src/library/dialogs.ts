import { create } from "zustand";
import type { Track } from "@/types";

// which library dialog is open; the dialogs themselves live in LibraryDialogs.tsx
interface DialogState {
  create: { tracks: Track[] } | null;
  importCode: boolean;
}

export const useLibraryDialogs = create<DialogState>(() => ({ create: null, importCode: false }));

export const openCreatePlaylist = (tracks: Track[] = []) => useLibraryDialogs.setState({ create: { tracks } });
export const openImportCode = () => useLibraryDialogs.setState({ importCode: true });
