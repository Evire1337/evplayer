import { create } from "zustand";

export type Page = "now" | "home" | "search" | "library" | "playlist";

interface NavState {
  page: Page;
  /** for "playlist": a playlist id, or "likes" */
  playlistId?: string;
  go: (page: Page, playlistId?: string) => void;
}

export const useNavigation = create<NavState>((set) => ({
  page: "home",
  go: (page, playlistId) => set({ page, playlistId }),
}));
