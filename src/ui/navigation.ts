import { create } from "zustand";

export type Page = "now" | "home" | "search" | "library" | "playlist" | "files" | "settings" | "artist" | "smart" | "collection";

interface NavState {
  page: Page;
  /** for "playlist": a playlist id or "likes"; "artist": an artist id; "smart": the kind */
  playlistId?: string;
  /** artist name, shown while the artist page loads */
  label?: string;
  back: Array<{ page: Page; playlistId?: string; label?: string }>;
  go: (page: Page, playlistId?: string, label?: string) => void;
  goBack: () => void;
}

export const useNavigation = create<NavState>((set) => ({
  page: "home",
  back: [],
  go: (page, playlistId, label) =>
    set((s) =>
      s.page === page && s.playlistId === playlistId
        ? s
        : { page, playlistId, label, back: [...s.back, { page: s.page, playlistId: s.playlistId, label: s.label }].slice(-30) },
    ),
  goBack: () =>
    set((s) => {
      const prev = s.back[s.back.length - 1];
      return prev ? { ...prev, back: s.back.slice(0, -1) } : s;
    }),
}));
