import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { Track } from "@/types";

// "Recently played": a track lands here after it has played for a few seconds, not only
// when it finished.

export const HISTORY_AFTER_MS = 5000;
const LIMIT = 60;

export interface HistoryEntry {
  track: Track;
  playedAt: string;
}

interface HistoryState {
  entries: HistoryEntry[];
  push: (track: Track, at?: Date) => void;
  clear: () => void;
}

export function pushEntry(entries: HistoryEntry[], track: Track, at: Date): HistoryEntry[] {
  return [{ track, playedAt: at.toISOString() }, ...entries.filter((e) => e.track.id !== track.id)].slice(0, LIMIT);
}

export const useHistory = create<HistoryState>()(
  persist(
    (set) => ({
      entries: [],
      push: (track, at = new Date()) => set((s) => ({ entries: pushEntry(s.entries, track, at) })),
      clear: () => set({ entries: [] }),
    }),
    { name: "evplayer.history.v1", storage: createJSONStorage(() => localStorage) },
  ),
);
