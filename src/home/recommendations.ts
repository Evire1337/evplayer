import { keeps } from "@/settings/store";
import type { Track } from "@/types";
import { similarTracks, songKey, uniqueSongs } from "@/player/radio";
import { getTopTracks } from "@/stats/listeningStats";
import type { LikedTrack } from "@/library/store";
import type { HistoryEntry } from "@/stats/history";

// Recommendations: radios of the songs you play most (and like most recently), mixed,
// without what you already know.

export interface Recommendations {
  forYou: Track[];
  because: Array<{ seed: Track; tracks: Track[] }>;
}

const TTL = 30 * 60_000;
let cached: { key: string; at: number; value: Recommendations } | null = null;

export function pickSeeds(likes: LikedTrack[], history: HistoryEntry[], limit = 4): Track[] {
  const seeds: Track[] = [];
  const add = (t: Track | undefined) => {
    if (t && !seeds.some((s) => s.id === t.id || songKey(s) === songKey(t))) seeds.push(t);
  };
  for (const row of getTopTracks("month", 3)) add(row.track);
  add(history[0]?.track);
  for (const l of likes.slice(0, 3)) add(l.track);
  return seeds.slice(0, limit);
}

export async function recommendations(likes: LikedTrack[], history: HistoryEntry[], signal?: AbortSignal): Promise<Recommendations> {
  const seeds = pickSeeds(likes, history);
  const key = seeds.map((s) => s.id).join(",");
  if (cached && cached.key === key && Date.now() - cached.at < TTL) return cached.value;
  const known = new Set<string>();
  for (const l of likes) {
    known.add(l.track.id);
    known.add(songKey(l.track));
  }
  for (const h of history) {
    known.add(h.track.id);
    known.add(songKey(h.track));
  }
  const lists = await Promise.all(seeds.map((s) => similarTracks(s, known, signal, 30).catch(() => [] as Track[])));
  const mixed: Track[] = [];
  for (let i = 0; i < 30; i++) for (const list of lists) if (list[i]) mixed.push(list[i]);
  const forYou = uniqueSongs(mixed).slice(0, 24);
  const used = new Set(forYou.slice(0, 12).map((t) => t.id));
  const because = seeds
    .slice(0, 2)
    .map((seed, i) => ({ seed, tracks: lists[i].filter((t) => !used.has(t.id)).slice(0, 14) }))
    .filter((b) => b.tracks.length >= 4);
  const value = { forYou, because };
  if (forYou.length && keeps("serviceCache")) cached = { key, at: Date.now(), value };
  return value;
}
