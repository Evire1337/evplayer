import type { Track } from "@/types";
import { searchYouTubeMusic } from "@/services/youtube/music";
import { searchSoundCloud } from "@/services/soundcloud/api";
import { bestMatch } from "@/player/match";

// Playlists from services we cannot play (Spotify, Deezer, a Spotify data export) carry only
// names. Each song is looked up on YouTube Music (the largest catalogue), then SoundCloud;
// songs without a confident match are left out instead of guessing.

export interface NamedTrack {
  title: string;
  artists: string[];
  durationMs?: number;
}

export interface MatchProgress {
  done: number;
  total: number;
  found: number;
}

function asTrack(t: NamedTrack): Track {
  return {
    id: `name:${t.title}`,
    source: "youtube",
    title: t.title,
    artists: t.artists.map((name) => ({ id: name, name })),
    durationMs: t.durationMs ?? 0,
  };
}

export async function matchOne(wanted: NamedTrack, signal?: AbortSignal): Promise<Track | null> {
  const query = `${wanted.artists[0] ?? ""} ${wanted.title}`.trim();
  const target = asTrack(wanted);
  const yt = await searchYouTubeMusic(query, signal).catch(() => [] as Track[]);
  const fromYt = bestMatch(target, yt.slice(0, 8), 0.65);
  if (fromYt) return fromYt;
  const sc = await searchSoundCloud(query, signal).catch(() => ({ tracks: [] as Track[] }));
  return bestMatch(target, sc.tracks.slice(0, 10), 0.65);
}

export async function matchTracks(
  wanted: NamedTrack[],
  options: { signal?: AbortSignal; onProgress?: (p: MatchProgress) => void; concurrency?: number } = {},
): Promise<Track[]> {
  const results: Array<Track | null> = new Array(wanted.length).fill(null);
  let next = 0;
  let done = 0;
  let found = 0;
  const worker = async () => {
    while (next < wanted.length && !options.signal?.aborted) {
      const i = next++;
      try {
        results[i] = await matchOne(wanted[i], options.signal);
      } catch {
        results[i] = null;
      }
      done++;
      if (results[i]) found++;
      options.onProgress?.({ done, total: wanted.length, found });
    }
  };
  await Promise.all(Array.from({ length: Math.min(options.concurrency ?? 4, wanted.length) }, worker));
  const seen = new Set<string>();
  return results.filter((t): t is Track => {
    if (!t || seen.has(t.id)) return false;
    seen.add(t.id);
    return true;
  });
}
