import type { Track } from "@/types";

// Lyrics from LRCLIB (https://lrclib.net), a free, open database of line-timed lyrics.

const API = "https://lrclib.net/api";

interface Record {
  trackName?: string;
  artistName?: string;
  duration?: number;
  instrumental?: boolean;
  plainLyrics?: string | null;
  syncedLyrics?: string | null;
}

export function normalizeTitle(value: string): string {
  return value
    .toLowerCase()
    .replace(/ё/g, "е")
    .replace(/[([{][^)\]}]*[)\]}]/g, " ")
    .replace(/\b(feat|ft|featuring|prod)\b\.?.*$/u, " ")
    .replace(/\b(official|video|audio|lyrics?|hd|hq|remaster(ed)?|клип)\b/g, " ")
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();
}

/** uploads often say "Artist - Title" in the title; try that reading too */
export function readings(track: Pick<Track, "title" | "artists">): Array<{ title: string; artist: string }> {
  const artist = (track.artists[0]?.name ?? "").replace(/\s*-\s*topic$/i, "").trim();
  const out = [{ title: track.title, artist }];
  const split = /^(.+?)\s+[-–—]\s+(.+)$/u.exec(track.title);
  if (split) out.push({ title: split[2], artist: split[1] });
  return out;
}

function score(record: Record, title: string, durationMs: number): number {
  if (record.instrumental || !(record.syncedLyrics || record.plainLyrics)) return -1;
  const want = normalizeTitle(title);
  const got = normalizeTitle(record.trackName ?? "");
  if (!want || !got) return -1;
  let s = want === got ? 60 : want.includes(got) || got.includes(want) ? 35 : -100;
  if (s < 0) return -1;
  if (durationMs && record.duration) {
    const diff = Math.abs(durationMs / 1000 - record.duration);
    if (diff > 20) return -1;
    s += diff <= 3 ? 30 : 10;
  }
  if (record.syncedLyrics) s += 10;
  return s;
}

export interface FoundLyrics {
  synced: boolean;
  text: string;
  source: string;
}

export async function findLyrics(track: Pick<Track, "title" | "artists" | "durationMs">, signal?: AbortSignal): Promise<FoundLyrics | null> {
  const candidates: Record[] = [];
  for (const r of readings(track)) {
    const params = new URLSearchParams({ track_name: r.title, artist_name: r.artist });
    const res = await fetch(`${API}/search?${params}`, { signal }).catch(() => null);
    if (res?.ok) candidates.push(...((await res.json()) as Record[]));
    if (candidates.some((c) => c.syncedLyrics)) break;
  }
  let best: { record: Record; title: string } | null = null;
  let bestScore = 0;
  for (const r of readings(track)) {
    for (const c of candidates) {
      const s = score(c, r.title, track.durationMs);
      if (s > bestScore) {
        bestScore = s;
        best = { record: c, title: r.title };
      }
    }
  }
  if (!best) return null;
  const synced = best.record.syncedLyrics?.trim();
  return synced
    ? { synced: true, text: synced, source: "LRCLIB" }
    : { synced: false, text: best.record.plainLyrics?.trim() ?? "", source: "LRCLIB" };
}
