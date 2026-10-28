import type { ArtistSummary, Track } from "@/types";

type Json = Record<string, unknown>;

const isObj = (v: unknown): v is Json => typeof v === "object" && v !== null && !Array.isArray(v);
const str = (v: unknown): string | undefined => (typeof v === "string" && v.trim() ? v.trim() : undefined);
const num = (v: unknown): number | undefined => (typeof v === "number" && Number.isFinite(v) ? v : undefined);

/** SoundCloud serves "-large" (100px) artwork by default; ask for the 500px version */
export function artworkUrl(raw: unknown, size = 500): string | undefined {
  const url = str(raw);
  // SoundCloud's grey placeholder has no bigger sizes (they 404): treat it as no picture
  if (!url || /default_avatar|placeholder/.test(url)) return undefined;
  return url ? url.replace(/-(large|t\d+x\d+|crop|original)(?=\.\w+$)/, `-t${size}x${size}`) : undefined;
}

export function mapScTrack(raw: unknown): Track | null {
  if (!isObj(raw) || raw.kind !== "track") return null;
  const id = num(raw.id);
  const title = str(raw.title);
  if (id === undefined || !title) return null;
  const user = isObj(raw.user) ? raw.user : undefined;
  const meta = isObj(raw.publisher_metadata) ? raw.publisher_metadata : undefined;
  const artistName = str(meta?.artist) ?? str(user?.username) ?? "SoundCloud";
  const userId = num(user?.id);
  return {
    id: `sc:${id}`,
    source: "soundcloud",
    title,
    artists: [{ id: userId !== undefined ? `scu:${userId}` : "unknown", name: artistName }],
    durationMs: num(raw.full_duration) ?? num(raw.duration) ?? 0,
    artwork: artworkUrl(raw.artwork_url) ?? artworkUrl(user?.avatar_url),
    plays: num(raw.playback_count),
    permalink: str(raw.permalink_url),
  };
}

export function mapScUser(raw: unknown): ArtistSummary | null {
  if (!isObj(raw) || raw.kind !== "user") return null;
  const id = num(raw.id);
  const name = str(raw.username);
  if (id === undefined || !name) return null;
  return { id: `scu:${id}`, name, avatar: artworkUrl(raw.avatar_url, 300), followers: num(raw.followers_count) };
}

export interface Transcoding {
  url: string;
  protocol: "progressive" | "hls";
  mimeType: string;
}

/** every format SoundCloud lists for a song, as "protocol mime" (to tell why none plays) */
export function formatsListed(raw: unknown): string[] {
  if (!isObj(raw) || !isObj(raw.media) || !Array.isArray(raw.media.transcodings)) return [];
  return raw.media.transcodings.flatMap((t) => {
    if (!isObj(t) || !isObj(t.format)) return [];
    const mime = (str(t.format.mime_type) ?? "").split(";")[0].replace("audio/", "");
    return [`${str(t.format.protocol) ?? "?"} ${mime}${t.snipped === true ? " (отрывок)" : ""}`];
  });
}

export function transcodingsOf(raw: unknown): Transcoding[] {
  if (!isObj(raw) || !isObj(raw.media) || !Array.isArray(raw.media.transcodings)) return [];
  return raw.media.transcodings.flatMap((t): Transcoding[] => {
    if (!isObj(t) || !isObj(t.format)) return [];
    const url = str(t.url);
    const protocol = str(t.format.protocol);
    const mimeType = str(t.format.mime_type) ?? "";
    if (!url || (protocol !== "progressive" && protocol !== "hls")) return [];
    // encrypted streams (premium "ctr-encrypted-hls") cannot be played here
    if (/encrypted/i.test(protocol) || t.snipped === true) return [];
    return [{ url, protocol, mimeType }];
  });
}

/**
 * best first: a single mp3 file, then plain HLS (mp3 before aac/opus because it works
 * everywhere); Opus in HLS comes last: its Ogg pieces are joined into one file by the app
 */
export function rankTranscodings(list: Transcoding[]): Transcoding[] {
  const rank = (t: Transcoding) =>
    (t.protocol === "progressive" ? 0 : 10) + (t.mimeType.includes("mpeg") ? 0 : t.mimeType.includes("mp4") ? 1 : 2);
  return [...list].sort((a, b) => rank(a) - rank(b));
}

export function pickTranscoding(list: Transcoding[]): Transcoding | undefined {
  return rankTranscodings(list)[0];
}

/** the per-track token SoundCloud wants on every stream request */
export function trackAuthorization(raw: unknown): string | undefined {
  return isObj(raw) ? str(raw.track_authorization) : undefined;
}

function firstTrackArtwork(tracks: unknown): string | undefined {
  if (!Array.isArray(tracks)) return undefined;
  for (const t of tracks) if (isObj(t) && t.artwork_url) return artworkUrl(t.artwork_url);
  return undefined;
}

export function mapScCollection(raw: unknown): import("@/types").CollectionSummary | null {
  if (!isObj(raw) || (raw.kind !== "playlist" && raw.kind !== "system-playlist")) return null;
  const id = num(raw.id);
  const title = str(raw.title);
  if (id === undefined || !title) return null;
  const user = isObj(raw.user) ? raw.user : undefined;
  const album = raw.is_album === true || str(raw.set_type) === "album" || str(raw.set_type) === "ep";
  const year = str(raw.release_date) ?? str(raw.published_at) ?? str(raw.created_at);
  const userId = num(user?.id);
  return {
    id: `scp:${id}`,
    kind: album ? "album" : "playlist",
    title,
    subtitle: str(user?.username) ?? "SoundCloud",
    // a playlist without its own picture shows its first song's cover on SoundCloud
    artwork: artworkUrl(raw.artwork_url) ?? firstTrackArtwork(raw.tracks),
    year: year ? new Date(year).getFullYear() : undefined,
    trackCount: num(raw.track_count),
    artist: userId !== undefined ? { id: `scu:${userId}`, name: str(user?.username) ?? "" } : undefined,
  };
}
