import type { Track } from "@/types";

// Lyrics from LRCLIB (https://lrclib.net), a free, open database of line-timed lyrics.

const API = "https://lrclib.net/api";

export interface Record {
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

/**
 * How far (seconds) a record's length may be from the song's for its line times to be trusted.
 * The lines are timed for one exact version: a version a few seconds longer or shorter (an
 * extended mix, a sped-up edit, a radio cut) has the text running ahead or behind the singer,
 * and the error grows toward the end. Past this the words are shown without times instead.
 */
export const SYNC_MAX_DIFF_S = 3;
/** the plain words are the same for versions this close in length */
const TEXT_MAX_DIFF_S = 20;

const lengthDiffS = (record: Record, durationMs: number): number | null => (durationMs && record.duration ? Math.abs(durationMs / 1000 - record.duration) : null);

/** whether the record's times can be followed for a song of this length (unknown length: yes) */
export function syncedFits(record: Record, durationMs: number): boolean {
  if (!record.syncedLyrics) return false;
  const diff = lengthDiffS(record, durationMs);
  return diff === null || diff <= SYNC_MAX_DIFF_S;
}

function score(record: Record, title: string, durationMs: number): number {
  if (record.instrumental || !(record.syncedLyrics || record.plainLyrics)) return -1;
  const want = normalizeTitle(title);
  const got = normalizeTitle(record.trackName ?? "");
  if (!want || !got) return -1;
  let s = want === got ? 60 : want.includes(got) || got.includes(want) ? 35 : -100;
  if (s < 0) return -1;
  const diff = lengthDiffS(record, durationMs);
  if (diff !== null) {
    if (diff > TEXT_MAX_DIFF_S) return -1;
    // the closest length wins: synced lines are timed for that exact version of the song,
    // and a version a second longer is usually a second off all the way through
    s += Math.max(0, 30 - diff * 6);
  }
  if (syncedFits(record, durationMs)) s += 10;
  return s;
}

/** the record that suits the song best, and whether its times can be followed */
export function pickRecord(candidates: Record[], track: Pick<Track, "title" | "artists" | "durationMs">): { record: Record; synced: boolean } | null {
  let best: Record | null = null;
  let bestScore = 0;
  for (const r of readings(track)) {
    for (const c of candidates) {
      const s = score(c, r.title, track.durationMs);
      if (s > bestScore) {
        bestScore = s;
        best = c;
      }
    }
  }
  return best ? { record: best, synced: syncedFits(best, track.durationMs) } : null;
}

/** the words of a timed text, without the times */
function stripTimes(text: string): string {
  return text
    .replace(/\[\d{1,3}:\d{1,2}(?:[.:]\d{1,3})?\]/g, "")
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .join("\n");
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
  if (!candidates.some((c) => c.syncedLyrics)) {
    // looser: one free-text query catches different spellings of the artist
    const r = readings(track)[readings(track).length - 1];
    const res = await fetch(`${API}/search?${new URLSearchParams({ q: `${r.artist} ${normalizeTitle(r.title)}` })}`, { signal }).catch(() => null);
    if (res?.ok) candidates.push(...((await res.json()) as Record[]));
  }
  const pick = pickRecord(candidates, track);
  if (!pick) return null;
  const { record, synced } = pick;
  // a version of another length: its words are right, its times are not
  if (synced && record.syncedLyrics?.trim()) return { synced: true, text: record.syncedLyrics.trim(), source: "LRCLIB" };
  const words = record.plainLyrics?.trim() || stripTimes(record.syncedLyrics ?? "");
  return words ? { synced: false, text: words, source: "LRCLIB" } : null;
}
