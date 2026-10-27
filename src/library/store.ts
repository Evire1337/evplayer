import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { Track } from "@/types";

// Everything you keep lives on this computer: likes and your own playlists. Tracks are
// stored whole (title, artists, artwork), so a playlist can mix SoundCloud, YouTube and
// your own files and still show instantly without asking any service.

export interface LikedTrack {
  track: Track;
  addedAt: string;
}

export interface PlaylistItem {
  id: string;
  track: Track;
  addedAt: string;
}

export interface Playlist {
  id: string;
  title: string;
  description?: string;
  /** your own picture, a small data: url */
  icon?: string;
  createdAt: string;
  updatedAt: string;
  items: PlaylistItem[];
}

interface LibraryState {
  likes: LikedTrack[];
  playlists: Playlist[];
  toggleLike: (track: Track) => void;
  moveLike: (trackId: string, toIndex: number) => void;
  createPlaylist: (title: string, description?: string) => Playlist;
  updatePlaylist: (id: string, patch: Partial<Pick<Playlist, "title" | "description" | "icon">>) => void;
  deletePlaylist: (id: string) => void;
  addToPlaylist: (playlistId: string, tracks: Track[]) => number;
  removeFromPlaylist: (playlistId: string, itemId: string) => void;
  moveInPlaylist: (playlistId: string, itemId: string, toIndex: number) => void;
}

const now = () => new Date().toISOString();
const newId = (prefix: string) => `${prefix}_${Date.now().toString(36)}${Math.random().toString(36).slice(2, 7)}`;

export function moveItem<T>(list: T[], from: number, to: number): T[] {
  if (from < 0 || from >= list.length || from === to) return list;
  const next = [...list];
  const [item] = next.splice(from, 1);
  next.splice(Math.max(0, Math.min(next.length, to)), 0, item);
  return next;
}

export const useLibrary = create<LibraryState>()(
  persist(
    (set, get) => ({
      likes: [],
      playlists: [],

      toggleLike: (track) =>
        set((s) =>
          s.likes.some((l) => l.track.id === track.id)
            ? { likes: s.likes.filter((l) => l.track.id !== track.id) }
            : { likes: [{ track, addedAt: now() }, ...s.likes] },
        ),

      moveLike: (trackId, toIndex) =>
        set((s) => ({ likes: moveItem(s.likes, s.likes.findIndex((l) => l.track.id === trackId), toIndex) })),

      createPlaylist: (title, description) => {
        const stamp = now();
        const playlist: Playlist = {
          id: newId("pl"),
          title: title.trim().slice(0, 60) || "Новый плейлист",
          ...(description?.trim() ? { description: description.trim().slice(0, 300) } : {}),
          createdAt: stamp,
          updatedAt: stamp,
          items: [],
        };
        set((s) => ({ playlists: [playlist, ...s.playlists] }));
        return playlist;
      },

      updatePlaylist: (id, patch) =>
        set((s) => ({
          playlists: s.playlists.map((p) => {
            if (p.id !== id) return p;
            const next: Playlist = { ...p, updatedAt: now() };
            if (patch.title !== undefined && patch.title.trim()) next.title = patch.title.trim().slice(0, 60);
            if (patch.description !== undefined) {
              if (patch.description.trim()) next.description = patch.description.trim().slice(0, 300);
              else delete next.description;
            }
            if (patch.icon !== undefined) {
              if (patch.icon) next.icon = patch.icon;
              else delete next.icon;
            }
            return next;
          }),
        })),

      deletePlaylist: (id) => set((s) => ({ playlists: s.playlists.filter((p) => p.id !== id) })),

      addToPlaylist: (playlistId, tracks) => {
        const playlist = get().playlists.find((p) => p.id === playlistId);
        if (!playlist) return 0;
        const have = new Set(playlist.items.map((i) => i.track.id));
        const fresh = tracks.filter((t) => !have.has(t.id));
        if (fresh.length === 0) return 0;
        const stamp = now();
        set((s) => ({
          playlists: s.playlists.map((p) =>
            p.id === playlistId
              ? { ...p, updatedAt: stamp, items: [...p.items, ...fresh.map((track) => ({ id: newId("pi"), track, addedAt: stamp }))] }
              : p,
          ),
        }));
        return fresh.length;
      },

      removeFromPlaylist: (playlistId, itemId) =>
        set((s) => ({
          playlists: s.playlists.map((p) => (p.id === playlistId ? { ...p, updatedAt: now(), items: p.items.filter((i) => i.id !== itemId) } : p)),
        })),

      moveInPlaylist: (playlistId, itemId, toIndex) =>
        set((s) => ({
          playlists: s.playlists.map((p) =>
            p.id === playlistId ? { ...p, updatedAt: now(), items: moveItem(p.items, p.items.findIndex((i) => i.id === itemId), toIndex) } : p,
          ),
        })),
    }),
    { name: "evplayer.library.v1", storage: createJSONStorage(() => localStorage) },
  ),
);

export const isLiked = (trackId: string) => useLibrary.getState().likes.some((l) => l.track.id === trackId);
export const useIsLiked = (trackId: string | undefined) => useLibrary((s) => Boolean(trackId) && s.likes.some((l) => l.track.id === trackId));
