import { keeps } from "@/settings/store";
import type { Track } from "@/types";

// Per-track listening statistics kept on this computer. Stored compactly (plays and
// listened time per day) so a year of history fits easily next to the library.

const STORAGE_KEY = "evplayer.stats.v1";
const SAVE_DEBOUNCE_MS = 500;
// days older than this are folded into a running total to keep the store small
const KEEP_DAYS = 400;

interface DayRow {
  n: number;
  ms: number;
}

export interface TrackStatsRecord {
  track: Track;
  days: Record<string, DayRow>;
  old: DayRow;
  first: string;
  last: string;
  sid?: string;
  sms?: number;
}

interface StatsData {
  version: 1;
  tracks: Record<string, TrackStatsRecord>;
}

let state: StatsData | null = null;
let saveTimer: ReturnType<typeof setTimeout> | null = null;

function load(): StatsData {
  if (state) return state;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const parsed = raw ? (JSON.parse(raw) as StatsData) : null;
    state = parsed?.version === 1 && parsed.tracks ? parsed : { version: 1, tracks: {} };
  } catch {
    state = { version: 1, tracks: {} };
  }
  return state;
}

function writeNow(): void {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = null;
  if (!state) return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // storage full or unavailable: statistics are a convenience, never fatal
  }
}

function scheduleSave(): void {
  if (saveTimer) return;
  saveTimer = setTimeout(writeNow, SAVE_DEBOUNCE_MS);
}

if (typeof window !== "undefined") window.addEventListener("beforeunload", writeNow);

export function dayKey(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function compact(record: TrackStatsRecord, now: Date): void {
  const cutoff = new Date(now);
  cutoff.setDate(cutoff.getDate() - KEEP_DAYS);
  const limit = dayKey(cutoff);
  for (const [day, row] of Object.entries(record.days)) {
    if (day < limit) {
      record.old.n += row.n;
      record.old.ms += row.ms;
      delete record.days[day];
    }
  }
}

/**
 * Record a listen. The player reports the same playback session more than once (after a
 * few seconds, then when the track ends), so a session counts as one play and only the
 * extra listened time is added.
 */
export function recordListen(track: Track, sessionId: string, playedMs: number, now = new Date()): void {
  if (!track?.id || !keeps("stats")) return;
  const data = load();
  const iso = now.toISOString();
  let record = data.tracks[track.id];
  if (!record) {
    record = { track, days: {}, old: { n: 0, ms: 0 }, first: iso, last: iso };
    data.tracks[track.id] = record;
  }
  record.track = track;
  record.last = iso;
  const day = dayKey(now);
  const row = (record.days[day] ??= { n: 0, ms: 0 });
  const sameSession = record.sid === sessionId;
  if (sameSession) {
    row.ms += Math.max(0, playedMs - (record.sms ?? 0));
  } else {
    row.n += 1;
    row.ms += Math.max(0, playedMs);
  }
  record.sms = sameSession ? Math.max(playedMs, record.sms ?? 0) : playedMs;
  record.sid = sessionId;
  compact(record, now);
  scheduleSave();
  if (typeof window !== "undefined") window.dispatchEvent(new Event("stats:changed"));
}

export type StatsRange = "week" | "month" | "all";

function rangeStart(range: StatsRange, now: Date): string | null {
  if (range === "all") return null;
  const start = new Date(now);
  start.setDate(start.getDate() - (range === "week" ? 6 : 29));
  return dayKey(start);
}

function sumRange(record: TrackStatsRecord, range: StatsRange, now: Date): DayRow {
  const start = rangeStart(range, now);
  const total: DayRow = start ? { n: 0, ms: 0 } : { ...record.old };
  for (const [day, row] of Object.entries(record.days)) {
    if (start && day < start) continue;
    total.n += row.n;
    total.ms += row.ms;
  }
  return total;
}

export interface TrackStatsSummary {
  week: DayRow;
  month: DayRow;
  all: DayRow;
  first?: string;
  last?: string;
  /** plays per day for the last 30 days, oldest first */
  daily: Array<{ day: string; n: number; ms: number }>;
  /** plays per month for the last 12 months, oldest first */
  monthly: Array<{ month: string; n: number; ms: number }>;
}

export function getTrackStats(trackId: string, now = new Date()): TrackStatsSummary {
  const record = load().tracks[trackId];
  const daily: TrackStatsSummary["daily"] = [];
  for (let i = 29; i >= 0; i--) {
    const date = new Date(now);
    date.setDate(date.getDate() - i);
    const key = dayKey(date);
    const row = record?.days[key];
    daily.push({ day: key, n: row?.n ?? 0, ms: row?.ms ?? 0 });
  }
  const monthly: TrackStatsSummary["monthly"] = [];
  for (let i = 11; i >= 0; i--) {
    const date = new Date(now.getFullYear(), now.getMonth() - i, 1);
    const prefix = dayKey(date).slice(0, 7);
    let n = 0;
    let ms = 0;
    for (const [day, row] of Object.entries(record?.days ?? {})) {
      if (day.startsWith(prefix)) {
        n += row.n;
        ms += row.ms;
      }
    }
    monthly.push({ month: prefix, n, ms });
  }
  if (!record) {
    const zero = { n: 0, ms: 0 };
    return { week: zero, month: zero, all: zero, daily, monthly };
  }
  return {
    week: sumRange(record, "week", now),
    month: sumRange(record, "month", now),
    all: sumRange(record, "all", now),
    first: record.first,
    last: record.last,
    daily,
    monthly,
  };
}

export interface TopTrack {
  track: Track;
  plays: number;
  ms: number;
}

export function getTopTracks(range: StatsRange, limit = 10, now = new Date()): TopTrack[] {
  return Object.values(load().tracks)
    .map((record) => {
      const total = sumRange(record, range, now);
      return { track: record.track, plays: total.n, ms: total.ms };
    })
    .filter((row) => row.plays > 0)
    .sort((a, b) => b.plays - a.plays || b.ms - a.ms)
    .slice(0, limit);
}

export interface TopArtist {
  id: string;
  name: string;
  plays: number;
  ms: number;
  cover?: string;
}

export function getTopArtists(range: StatsRange, limit = 10, now = new Date()): TopArtist[] {
  const byArtist = new Map<string, TopArtist>();
  for (const record of Object.values(load().tracks)) {
    const total = sumRange(record, range, now);
    if (!total.n) continue;
    for (const artist of record.track.artists ?? []) {
      const name = artist.name?.trim();
      if (!name || name === "Unknown Artist") continue;
      const key = artist.id && artist.id !== "unknown" ? artist.id : `name:${name.toLowerCase()}`;
      const row = byArtist.get(key) ?? { id: artist.id, name, plays: 0, ms: 0, cover: record.track.artwork };
      row.plays += total.n;
      row.ms += total.ms;
      byArtist.set(key, row);
    }
  }
  return [...byArtist.values()].sort((a, b) => b.plays - a.plays || b.ms - a.ms).slice(0, limit);
}

/** first run with statistics: count the plays already in the listening history once */
export function seedFromHistory(rows: Array<{ playbackSessionId: string; track: Track; playedDurationMs: number; listenedAt: string }>): void {
  const data = load() as StatsData & { seeded?: boolean };
  if (data.seeded) return;
  data.seeded = true;
  if (Object.keys(data.tracks).length === 0) {
    for (const row of [...rows].reverse()) {
      if (row.track?.id) recordListen(row.track, row.playbackSessionId, row.playedDurationMs, new Date(row.listenedAt));
    }
  }
  scheduleSave();
}

export function resetListeningStatsForTests(): void {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = null;
  state = null;
}

export function flushListeningStats(): void {
  writeNow();
}

// ---- smart playlists ----------------------------------------------------------------

export type SmartPlaylistKind = "on_repeat" | "forgotten" | "discoveries" | "liked_rare";

export const SMART_PLAYLISTS: SmartPlaylistKind[] = ["on_repeat", "discoveries", "forgotten", "liked_rare"];

const DAY_MS = 86_400_000;

/**
 * Playlists that fill themselves from your listening:
 *  on_repeat    played 3+ times in the last 30 days
 *  discoveries  first heard in the last 14 days
 *  forgotten    played 3+ times overall, but not in the last 45 days
 *  liked_rare   liked, but played at most once
 */
export function getSmartPlaylist(kind: SmartPlaylistKind, liked: Track[] = [], now = new Date(), limit = 100): Track[] {
  const records = Object.values(load().tracks);
  const time = now.getTime();
  switch (kind) {
    case "on_repeat":
      return records
        .map((r) => ({ r, n: sumRange(r, "month", now).n }))
        .filter((x) => x.n >= 3)
        .sort((a, b) => b.n - a.n)
        .slice(0, limit)
        .map((x) => x.r.track);
    case "discoveries":
      return records
        .filter((r) => time - new Date(r.first).getTime() <= 14 * DAY_MS)
        .map((r) => ({ r, n: sumRange(r, "all", now).n }))
        .sort((a, b) => b.n - a.n)
        .slice(0, limit)
        .map((x) => x.r.track);
    case "forgotten":
      return records
        .map((r) => ({ r, n: sumRange(r, "all", now).n }))
        .filter((x) => x.n >= 3 && time - new Date(x.r.last).getTime() > 45 * DAY_MS)
        .sort((a, b) => b.n - a.n)
        .slice(0, limit)
        .map((x) => x.r.track);
    case "liked_rare": {
      const data = load().tracks;
      return liked.filter((t) => !data[t.id] || sumRange(data[t.id], "all", now).n <= 1).slice(0, limit);
    }
  }
}

/** how long you listened today, all songs together */
export function listenedToday(now = new Date()): number {
  const key = dayKey(now);
  let ms = 0;
  for (const record of Object.values(load().tracks)) ms += record.days[key]?.ms ?? 0;
  return ms;
}

export function clearListeningStats(): void {
  state = { version: 1, tracks: {} };
  writeNow();
  if (typeof window !== "undefined") window.dispatchEvent(new Event("stats:changed"));
}

/** everything, for a backup */
export function exportListeningStats(): unknown {
  return load();
}

export function importListeningStats(data: unknown): boolean {
  const parsed = data as StatsData | null;
  if (!parsed || parsed.version !== 1 || typeof parsed.tracks !== "object") return false;
  state = parsed;
  writeNow();
  if (typeof window !== "undefined") window.dispatchEvent(new Event("stats:changed"));
  return true;
}
