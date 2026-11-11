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
    ...(isPreviewOnly(raw) && { preview: true }),
  };
}

/** SoundCloud gives this song out only as a 30-second preview (a SoundCloud Go+ song) */
export function isPreviewOnly(raw: Json): boolean {
  if (str(raw.policy) === "SNIP") return true;
  const full = num(raw.full_duration);
  const given = num(raw.duration);
  return full !== undefined && given !== undefined && given <= PREVIEW_MS && full > given + 5000;
}

/** previews are 30 seconds long */
const PREVIEW_MS = 31_000;

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
  /** SoundCloud's name of the encoding: "mp3_1_0", "opus_0_0", "aac_160k" */
  preset?: string;
  /** "hq" for the better encoding (Go+ songs), "sq" for the standard one */
  quality?: string;
}

/** what a person is told about the sound: "mp3 ~128 кбит/с" (SoundCloud does not say the exact rate) */
export function qualityOf(t: Pick<Transcoding, "mimeType" | "preset">): string {
  const codec = /opus|ogg/i.test(t.mimeType) ? "opus" : /mp4|aac/i.test(t.mimeType) || /^aac/i.test(t.preset ?? "") ? "aac" : "mp3";
  const named = /(\d{2,3})k/i.exec(t.preset ?? "")?.[1];
  const rate = named ?? { mp3: "128", opus: "64", aac: "160" }[codec];
  return `${codec} ${named ? "" : "~"}${rate} кбит/с`;
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

/** a format that is a 30-second cut of a longer song */
function snippedBy(track: Json, t: Json): boolean {
  const full = num(track.full_duration);
  const length = num(t.duration);
  return full !== undefined && length !== undefined && length <= PREVIEW_MS && full > length + 5000;
}

export function transcodingsOf(raw: unknown): Transcoding[] {
  if (!isObj(raw) || !isObj(raw.media) || !Array.isArray(raw.media.transcodings)) return [];
  return raw.media.transcodings.flatMap((t): Transcoding[] => {
    if (!isObj(t) || !isObj(t.format)) return [];
    const url = str(t.url);
    const protocol = str(t.format.protocol);
    const mimeType = str(t.format.mime_type) ?? "";
    if (!url || (protocol !== "progressive" && protocol !== "hls")) return [];
    // encrypted streams (premium "ctr-encrypted-hls") cannot be played here, and a preview
    // is not the song (some are not marked as one: they are told by their length)
    if (/encrypted/i.test(protocol) || t.snipped === true || snippedBy(raw, t)) return [];
    return [{ url, protocol, mimeType, ...(str(t.preset) && { preset: str(t.preset) }), ...(str(t.quality) && { quality: str(t.quality) }) }];
  });
}

/**
 * best first: a single mp3 file, then plain HLS (mp3 before aac/opus because it works
 * everywhere); Opus in HLS comes last: its Ogg pieces are joined into one file by the app
 */
export function rankTranscodings(list: Transcoding[]): Transcoding[] {
  // the better encoding of a song comes first when SoundCloud offers one (Go+); then as before
  const rank = (t: Transcoding) =>
    (t.quality === "hq" ? 0 : 100) + (t.protocol === "progressive" ? 0 : 10) + (t.mimeType.includes("mpeg") ? 0 : t.mimeType.includes("mp4") ? 1 : 2);
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
