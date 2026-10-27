import { keeps } from "@/settings/store";
import type { Track } from "@/types";
import { searchSoundCloud, soundCloudRelated } from "@/services/soundcloud/api";
import { searchYouTubeMusic, youTubeRadio } from "@/services/youtube/music";
import { searchQueryFor, simplify, splitTitle } from "./match";

// Similar songs: YouTube Music's own radio ("up next") for the song, plus SoundCloud's
// related tracks for SoundCloud songs. Used for autoplay, "track radio" and the
// recommendations on the home page.

function mix(a: Track[], b: Track[]): Track[] {
  const out: Track[] = [];
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if (a[i]) out.push(a[i]);
    if (b[i]) out.push(b[i]);
  }
  return out;
}

/** the same song uploaded twice counts once */
export function songKey(t: Track): string {
  const { title, artist } = splitTitle(t);
  return `${simplify(artist)}|${simplify(title)}`;
}

export function uniqueSongs(list: Track[], exclude: Set<string> = new Set()): Track[] {
  const seenKeys = new Set<string>();
  return list.filter((t) => {
    if (exclude.has(t.id)) return false;
    const key = songKey(t);
    if (seenKeys.has(key) || exclude.has(key)) return false;
    seenKeys.add(key);
    return true;
  });
}

async function youTubeSeed(seed: Track, signal?: AbortSignal): Promise<string | null> {
  if (seed.source === "youtube") return seed.id;
  const found = await searchYouTubeMusic(searchQueryFor(seed), signal).catch(() => [] as Track[]);
  return found[0]?.id ?? null;
}

const cache = new Map<string, { at: number; tracks: Track[] }>();
const TTL = 30 * 60_000;

export async function similarTracks(seed: Track, exclude: Set<string> = new Set(), signal?: AbortSignal, limit = 25): Promise<Track[]> {
  let all = cache.get(seed.id)?.at && Date.now() - cache.get(seed.id)!.at < TTL ? cache.get(seed.id)!.tracks : null;
  if (!all) {
    const [yt, sc] = await Promise.all([
      youTubeSeed(seed, signal).then((id) => (id ? youTubeRadio(id, signal) : [])).catch(() => [] as Track[]),
      seed.source === "soundcloud"
        ? soundCloudRelated(seed.id, signal).catch(() => [] as Track[])
        : Promise.resolve([] as Track[]),
    ]);
    all = mix(yt, sc);
    if (!all.length && seed.artists[0]) {
      // last resort: more by the same artist
      const name = simplify(seed.artists[0].name);
      const [y, s] = await Promise.all([
        searchYouTubeMusic(name, signal).catch(() => [] as Track[]),
        searchSoundCloud(name, signal).then((r) => r.tracks).catch(() => [] as Track[]),
      ]);
      all = mix(y, s);
    }
    if (all.length && keeps("serviceCache")) cache.set(seed.id, { at: Date.now(), tracks: all });
  }
  const skip = new Set([...exclude, seed.id, songKey(seed)]);
  return uniqueSongs(all, skip).slice(0, limit);
}

/** what the remembered similar songs take, roughly, in bytes */
export function radioCacheBytes(): number {
  let n = 0;
  for (const v of cache.values()) n += JSON.stringify(v).length * 2;
  return n;
}
