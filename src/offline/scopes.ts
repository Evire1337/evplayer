import type { Track } from "@/types";
import { useLibrary } from "@/library/store";
import { useOffline } from "./store";

// "Scopes" are the lists that can be kept offline as a whole: the likes ("likes") or one of
// your playlists (its id). Tracks of your own files are already on disk and never listed.

export const LIKES_SCOPE = "likes";

/** services' songs only: files from this computer need no copy */
export const isKeepable = (track: Track) => track.source === "soundcloud" || track.source === "youtube";

export function scopeTitle(scope: string): string {
  if (scope === LIKES_SCOPE) return "Понравившиеся";
  return useLibrary.getState().playlists.find((p) => p.id === scope)?.title ?? "Плейлист";
}

/** the songs of a scope that can be kept, without repeats */
export function scopeTracks(scope: string): Track[] {
  const lib = useLibrary.getState();
  const all = scope === LIKES_SCOPE ? lib.likes.map((l) => l.track) : (lib.playlists.find((p) => p.id === scope)?.items.map((i) => i.track) ?? []);
  const seen = new Set<string>();
  return all.filter((t) => isKeepable(t) && !seen.has(t.id) && seen.add(t.id) !== undefined);
}

export interface ScopeStatus {
  total: number;
  kept: number;
}

export function scopeStatus(scope: string, kept: Record<string, unknown> = useOffline.getState().entries): ScopeStatus {
  const tracks = scopeTracks(scope);
  return { total: tracks.length, kept: tracks.filter((t) => t.id in kept).length };
}

/** every song of several scopes, once */
export function tracksOf(scopes: string[]): Track[] {
  const seen = new Set<string>();
  return scopes.flatMap(scopeTracks).filter((t) => !seen.has(t.id) && seen.add(t.id) !== undefined);
}

/** the kept songs the library knows by name (the likes and every playlist), once each */
export function keptTracks(kept: Record<string, unknown> = useOffline.getState().entries): Track[] {
  const scopes = [LIKES_SCOPE, ...useLibrary.getState().playlists.map((p) => p.id)];
  return tracksOf(scopes).filter((t) => t.id in kept);
}
