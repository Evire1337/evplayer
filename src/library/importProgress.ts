import { create } from "zustand";

// one long import at a time (a link or a Spotify export): shown as a pill with a cancel button

interface ImportProgress {
  text: string | null;
  cancel: (() => void) | null;
  start: (text: string, cancel: () => void) => void;
  update: (text: string) => void;
  finish: () => void;
}

export const useImportProgress = create<ImportProgress>((set) => ({
  text: null,
  cancel: null,
  start: (text, cancel) => set({ text, cancel }),
  update: (text) => set((s) => (s.text === null ? s : { text })),
  finish: () => set({ text: null, cancel: null }),
}));
