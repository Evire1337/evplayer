import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { OfflineEntry } from "@/platform/api";

// What is kept on this computer for listening without a connection. The desktop side owns
// the files and the index; this mirrors the index for the page and adds what only the page
// knows: how far each song in flight has got, and which lists are kept up to date.

interface OfflineState {
  /** songs that are on disk, by track id */
  entries: Record<string, OfflineEntry>;
  /** songs being fetched right now: 0..1 (or -1 while the length is unknown) */
  progress: Record<string, number>;
  loaded: boolean;
  refresh: () => Promise<void>;
}

const bridge = () => window.evplayer?.offline;

export const offlineSupported = () => Boolean(bridge());

export const useOffline = create<OfflineState>((set) => ({
  entries: {},
  progress: {},
  loaded: false,
  refresh: async () => {
    const list = (await bridge()?.list()) ?? [];
    set({ entries: Object.fromEntries(list.map((e) => [e.id, e])), loaded: true });
  },
}));

/** where a kept song plays from, or null when it is not on disk */
export function keptUrl(trackId: string): string | null {
  return useOffline.getState().entries[trackId] ? (bridge()?.mediaUrl(trackId) ?? null) : null;
}

export const useIsKept = (trackId: string) => useOffline((s) => trackId in s.entries);

export function keptBytes(entries: Record<string, OfflineEntry>): number {
  return Object.values(entries).reduce((sum, e) => sum + e.size, 0);
}

/**
 * Lists whose new songs are fetched on their own ("likes" or a playlist id). Remembered on
 * this computer; the songs themselves are in the offline folder.
 */
interface KeepState {
  lists: string[];
  /**
   * Songs that could not be kept (no format that plays, removed from the service). The lists
   * are not topped up with them again on every start; asking for one by hand tries again.
   */
  failed: string[];
  has: (scope: string) => boolean;
  add: (scopes: string[]) => void;
  remove: (scopes: string[]) => void;
}

export const useKeepLists = create<KeepState>()(
  persist(
    (set, get) => ({
      lists: [],
      failed: [],
      has: (scope) => get().lists.includes(scope),
      add: (scopes) => set((s) => ({ lists: [...new Set([...s.lists, ...scopes])] })),
      remove: (scopes) => set((s) => ({ lists: s.lists.filter((l) => !scopes.includes(l)) })),
    }),
    { name: "evplayer.offline.v1", storage: createJSONStorage(() => localStorage), partialize: (s) => ({ lists: s.lists, failed: s.failed }) },
  ),
);
