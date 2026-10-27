import { keeps } from "@/settings/store";
import type { Track } from "@/types";
import { soundCloudUser } from "@/services/soundcloud/api";
import { youTubeArtist } from "@/services/youtube/music";
import type { LikedTrack } from "@/library/store";

// "New from your artists": the artists that show up most in your likes, and their newest
// uploads that you have not liked yet.

const MAX_ARTISTS = 8;
const PER_ARTIST = 2;
const TTL = 30 * 60_000;
let cached: { key: string; at: number; tracks: Track[] } | null = null;

export function topLikedArtists(likes: LikedTrack[], limit = MAX_ARTISTS): Array<{ id: string; name: string }> {
  const counts = new Map<string, { id: string; name: string; n: number }>();
  for (const { track } of likes) {
    for (const artist of track.artists) {
      if (!/^(scu|yta):/.test(artist.id)) continue;
      const row = counts.get(artist.id) ?? { id: artist.id, name: artist.name, n: 0 };
      row.n++;
      counts.set(artist.id, row);
    }
  }
  return [...counts.values()].sort((a, b) => b.n - a.n).slice(0, limit);
}

export async function newFromArtists(likes: LikedTrack[], signal?: AbortSignal): Promise<Track[]> {
  const artists = topLikedArtists(likes);
  const key = artists.map((a) => a.id).join(",");
  if (cached && cached.key === key && Date.now() - cached.at < TTL) return cached.tracks;
  const liked = new Set(likes.map((l) => l.track.id));
  const groups = await Promise.all(
    artists.map(async (a) => {
      try {
        const tracks = a.id.startsWith("scu:") ? (await soundCloudUser(a.id, signal)).tracks : (await youTubeArtist(a.id, signal)).tracks;
        return tracks.filter((t) => !liked.has(t.id)).slice(0, PER_ARTIST);
      } catch {
        return [];
      }
    }),
  );
  // take turns so one busy artist does not fill the whole row
  const out: Track[] = [];
  for (let i = 0; i < PER_ARTIST; i++) for (const g of groups) if (g[i]) out.push(g[i]);
  if (keeps("serviceCache")) cached = { key, at: Date.now(), tracks: out };
  return out;
}

/** what the remembered answer takes, roughly, in bytes */
export function newFromArtistsCacheBytes(): number {
  return cached ? JSON.stringify(cached).length * 2 : 0;
}
