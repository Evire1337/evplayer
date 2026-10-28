import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { Track } from "@/types";
import { sameSource, sourceKey, type PlaySource } from "./playSource";

// "Recently played": a track lands here after it has played for a few seconds, not only
// when it finished.

export const HISTORY_AFTER_MS = 5000;
const LIMIT = 60;

export interface HistoryEntry {
  track: Track;
  playedAt: string;
  /** `sourceKey` of the playlist or album it was played from */
  from?: string;
}

/** a playlist, album or other list a queue was played from */
export interface SourceEntry {
  source: PlaySource;
  playedAt: string;
}

const SOURCE_LIMIT = 20;

interface HistoryState {
  entries: HistoryEntry[];
  sources: SourceEntry[];
  push: (track: Track, at?: Date, from?: PlaySource | null) => void;
  clear: () => void;
}

export function pushEntry(entries: HistoryEntry[], track: Track, at: Date, from?: PlaySource | null): HistoryEntry[] {
  return [{ track, playedAt: at.toISOString(), ...(from ? { from: sourceKey(from) } : {}) }, ...entries.filter((e) => e.track.id !== track.id)].slice(0, LIMIT);
}

export function pushSourceEntry(sources: SourceEntry[], source: PlaySource, at: Date): SourceEntry[] {
  return [{ source, playedAt: at.toISOString() }, ...sources.filter((e) => !sameSource(e.source, source))].slice(0, SOURCE_LIMIT);
}

export const useHistory = create<HistoryState>()(
  persist(
    (set) => ({
      entries: [],
      sources: [],
      // a song from a playlist also brings the playlist to the front
      push: (track, at = new Date(), from) =>
        set((s) => ({ entries: pushEntry(s.entries, track, at, from), sources: from ? pushSourceEntry(s.sources, from, at) : s.sources })),
      clear: () => set({ entries: [], sources: [] }),
    }),
    { name: "evplayer.history.v1", storage: createJSONStorage(() => localStorage) },
  ),
);
