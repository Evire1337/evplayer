import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { ArtistSummary, CollectionSummary, Track } from "@/types";
import type { SearchSource } from "@/settings/store";
import { keeps } from "@/settings/store";

// The search page keeps what it showed: opening a song's player (or any other tab) and
// coming back finds the same query, filters and results, without searching again.

export type SearchResult = { tracks: Track[]; artists: ArtistSummary[]; albums: CollectionSummary[]; playlists: CollectionSummary[]; failed: string[] };
export type SearchKind = "all" | "tracks" | "artists" | "albums" | "playlists";
export const EMPTY_RESULT: SearchResult = { tracks: [], artists: [], albums: [], playlists: [], failed: [] };

interface SearchState {
  query: string;
  source: SearchSource | null;
  kind: SearchKind;
  result: SearchResult;
  status: "idle" | "loading" | "ready" | "error";
  /** what `result` was found for, so coming back does not search again */
  resultKey: string;
  set: (patch: Partial<Omit<SearchState, "set">>) => void;
}

export const useSearchState = create<SearchState>((set) => ({
  query: "",
  source: null,
  kind: "all",
  result: EMPTY_RESULT,
  status: "idle",
  resultKey: "",
  set: (patch) => set(patch),
}));

export const searchKey = (q: string, source: SearchSource, kind: SearchKind) => `${source}|${kind}|${q.trim().toLowerCase()}`;

// ---- history -----------------------------------------------------------------------------

const HISTORY_LIMIT = 20;

interface SearchHistory {
  queries: string[];
  add: (query: string) => void;
  remove: (query: string) => void;
  clear: () => void;
}

export const useSearchHistory = create<SearchHistory>()(
  persist(
    (set) => ({
      queries: [],
      add: (query) => {
        const q = query.trim();
        // "Не сохранять историю" covers searches too
        if (q.length < 2 || !keeps("history")) return;
        set((s) => ({ queries: [q, ...s.queries.filter((x) => x.toLowerCase() !== q.toLowerCase())].slice(0, HISTORY_LIMIT) }));
      },
      remove: (query) => set((s) => ({ queries: s.queries.filter((x) => x !== query) })),
      clear: () => set({ queries: [] }),
    }),
    { name: "evplayer.searchHistory.v1", storage: createJSONStorage(() => localStorage) },
  ),
);
