import type { CollectionSummary, Track } from "@/types";

// Search on YouTube Music: the same public search the music.youtube.com page uses.
// Results become ordinary tracks with ids "yt:VIDEO_ID".

const ENDPOINT = "https://music.youtube.com/youtubei/v1";
const CLIENT = { clientName: "WEB_REMIX", clientVersion: "1.20241023.01.00", hl: "ru", gl: "US" };

// "Songs" filter; not every country has it, then the "Videos" filter is used
const SONGS = "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D";
const VIDEOS = "EgWKAQIQAWoKEAQQBRAVEAoQEQ%3D%3D";

interface Run {
  text: string;
  navigationEndpoint?: {
    watchEndpoint?: { videoId?: string };
    browseEndpoint?: { browseId?: string; browseEndpointContextSupportedConfigs?: { browseEndpointContextMusicConfig?: { pageType?: string } } };
  };
}

type Json = Record<string, unknown>;

function* findAll(node: unknown, key: string): Generator<Json> {
  if (Array.isArray(node)) {
    for (const item of node) yield* findAll(item, key);
  } else if (node && typeof node === "object") {
    const obj = node as Json;
    if (key in obj) yield obj[key] as Json;
    for (const value of Object.values(obj)) yield* findAll(value, key);
  }
}

export function parseDuration(text: string): number {
  if (!/^\d+(:\d{1,2}){1,2}$/.test(text.trim())) return 0;
  return text.trim().split(":").reduce((acc, part) => acc * 60 + Number(part), 0) * 1000;
}

/** "58 млн просмотров", "1.2M views", "385 тыс." -> a number */
export function parseCount(text: string): number | undefined {
  const m = /([\d.,\s ]+)\s*(млрд|млн|тыс|B|M|K|bn|mln)?/i.exec(text.replace(/ /g, " "));
  if (!m || !/\d/.test(m[1])) return undefined;
  const value = Number(m[1].replace(/\s/g, "").replace(",", "."));
  if (!Number.isFinite(value)) return undefined;
  const unit = (m[2] ?? "").toLowerCase();
  const mult = unit === "млрд" || unit === "b" || unit === "bn" ? 1e9 : unit === "млн" || unit === "m" || unit === "mln" ? 1e6 : unit === "тыс" || unit === "k" ? 1e3 : 1;
  return Math.round(value * mult);
}

function bestThumb(item: Json): string | undefined {
  const thumbs = [...findAll(item.thumbnail, "thumbnails")][0] as unknown as Array<{ url: string; width: number }> | undefined;
  if (!thumbs?.length) return undefined;
  const url = thumbs[thumbs.length - 1].url;
  // album covers come as lh3 urls with a size suffix: ask for a bigger one
  return url.replace(/=w\d+-h\d+/, "=w544-h544");
}

export function parseListItem(item: Json): Track | null {
  const columns = (item.flexColumns as Json[] | undefined) ?? [];
  const runsOf = (i: number): Run[] =>
    ((columns[i]?.musicResponsiveListItemFlexColumnRenderer as Json | undefined)?.text as { runs?: Run[] } | undefined)?.runs ?? [];
  const titleRuns = runsOf(0);
  const videoId =
    titleRuns.find((r) => r.navigationEndpoint?.watchEndpoint?.videoId)?.navigationEndpoint?.watchEndpoint?.videoId ??
    (item.playlistItemData as { videoId?: string } | undefined)?.videoId;
  if (!videoId || !titleRuns.length) return null;
  const meta = [...runsOf(1), ...runsOf(2)];
  const artists = meta
    .filter((r) => {
      const page = r.navigationEndpoint?.browseEndpoint?.browseEndpointContextSupportedConfigs?.browseEndpointContextMusicConfig?.pageType;
      return page === "MUSIC_PAGE_TYPE_ARTIST" || page === "MUSIC_PAGE_TYPE_USER_CHANNEL";
    })
    .map((r) => ({ id: `yta:${r.navigationEndpoint!.browseEndpoint!.browseId}`, name: r.text }));
  const texts = meta.map((r) => r.text.trim()).filter((t) => t && t !== "•");
  if (!artists.length && texts[0] && !/^\d/.test(texts[0])) artists.push({ id: `ytn:${texts[0]}`, name: texts[0] });
  let durationMs = 0;
  let plays: number | undefined;
  for (const text of texts) {
    if (!durationMs && parseDuration(text)) durationMs = parseDuration(text);
    else if (plays === undefined && /\d/.test(text) && /(просм|views|прослуш|plays|воспр)/i.test(text)) plays = parseCount(text);
  }
  const fixed = (item.fixedColumns as Json[] | undefined)?.[0]?.musicResponsiveListItemFixedColumnRenderer as Json | undefined;
  const fixedText = (fixed?.text as { runs?: Run[] } | undefined)?.runs?.[0]?.text;
  if (!durationMs && fixedText) durationMs = parseDuration(fixedText);
  return {
    id: `yt:${videoId}`,
    source: "youtube",
    title: titleRuns.map((r) => r.text).join(""),
    artists,
    durationMs,
    artwork: bestThumb(item),
    plays,
    permalink: `https://music.youtube.com/watch?v=${videoId}`,
  };
}

export function parseSearch(response: unknown): Track[] {
  const out: Track[] = [];
  const seen = new Set<string>();
  for (const item of findAll(response, "musicResponsiveListItemRenderer")) {
    const track = parseListItem(item);
    if (track && !seen.has(track.id)) {
      seen.add(track.id);
      out.push(track);
    }
  }
  return out;
}

async function call(path: string, body: Json, signal?: AbortSignal): Promise<unknown> {
  const res = await fetch(`${ENDPOINT}/${path}?prettyPrint=false`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ context: { client: CLIENT }, ...body }),
    signal,
  });
  if (!res.ok) throw new Error(`YouTube Music ответил ${res.status}`);
  return res.json();
}

export async function searchYouTubeMusic(query: string, signal?: AbortSignal): Promise<Track[]> {
  const songs = parseSearch(await call("search", { query, params: SONGS }, signal));
  if (songs.length) return songs;
  return parseSearch(await call("search", { query, params: VIDEOS }, signal));
}

const ALBUMS = "EgWKAQIYAWoKEAkQAxAEEAkQBQ%3D%3D";
const PLAYLISTS = "EgeKAQQoAEABagoQBBAFEBUQChAR";

/** an album or playlist row in search results */
export function parseCollectionItem(item: Json): CollectionSummary | null {
  const browse = (item.navigationEndpoint as Run["navigationEndpoint"] | undefined)?.browseEndpoint;
  const page = browse?.browseEndpointContextSupportedConfigs?.browseEndpointContextMusicConfig?.pageType;
  if (!browse?.browseId || (page !== "MUSIC_PAGE_TYPE_ALBUM" && page !== "MUSIC_PAGE_TYPE_PLAYLIST")) return null;
  const columns = (item.flexColumns as Json[] | undefined) ?? [];
  const runsOf = (i: number): Run[] =>
    ((columns[i]?.musicResponsiveListItemFlexColumnRenderer as Json | undefined)?.text as { runs?: Run[] } | undefined)?.runs ?? [];
  const title = runsOf(0).map((r) => r.text).join("");
  if (!title) return null;
  const meta = runsOf(1);
  const texts = meta.map((r) => r.text.trim()).filter((t) => t && t !== "•");
  const artistRun = meta.find((r) => r.navigationEndpoint?.browseEndpoint?.browseId);
  const year = texts.map(Number).find((n) => n > 1900 && n < 2100);
  return {
    id: `ytb:${browse.browseId}`,
    kind: page === "MUSIC_PAGE_TYPE_ALBUM" ? "album" : "playlist",
    title,
    subtitle: texts.join(" · "),
    artwork: bestThumb(item),
    year,
    artist: artistRun ? { id: `yta:${artistRun.navigationEndpoint!.browseEndpoint!.browseId}`, name: artistRun.text } : undefined,
  };
}

export function parseCollections(response: unknown): CollectionSummary[] {
  const out: CollectionSummary[] = [];
  for (const item of findAll(response, "musicResponsiveListItemRenderer")) {
    const c = parseCollectionItem(item);
    if (c && !out.some((x) => x.id === c.id)) out.push(c);
  }
  return out;
}

export async function searchYouTubeCollections(query: string, kind: "album" | "playlist", signal?: AbortSignal): Promise<CollectionSummary[]> {
  return parseCollections(await call("search", { query, params: kind === "album" ? ALBUMS : PLAYLISTS }, signal));
}

export interface YouTubeCollection {
  title: string;
  subtitle?: string;
  artwork?: string;
  artist?: { id: string; name: string };
  tracks: Track[];
}

/** an album (MPREb…) or playlist (VL…) page with all its songs */
export async function youTubeCollection(browseId: string, signal?: AbortSignal, limit = 500): Promise<YouTubeCollection> {
  const id = browseId.replace(/^ytb:/, "");
  let response = await call("browse", { browseId: /^(MPRE|VL|OLAK)/.test(id) ? id : `VL${id}` }, signal);
  const header = ([...findAll(response, "musicResponsiveHeaderRenderer")][0] ?? [...findAll(response, "musicDetailHeaderRenderer")][0] ?? {}) as Json;
  const title = (header.title as { runs?: Run[] } | undefined)?.runs?.map((r) => r.text).join("") ?? "YouTube";
  const strap = (header.straplineTextOne as { runs?: Run[] } | undefined)?.runs ?? [];
  const artistRun = strap.find((r) => r.navigationEndpoint?.browseEndpoint?.browseId);
  const subtitle = ((header.subtitle as { runs?: Run[] } | undefined)?.runs ?? []).map((r) => r.text).join("");
  const thumbs = [...findAll(header, "thumbnails")][0] as unknown as Array<{ url: string }> | undefined;
  const artwork = thumbs?.[thumbs.length - 1]?.url?.replace(/=w\d+-h\d+/, "=w544-h544");
  const artist = artistRun ? { id: `yta:${artistRun.navigationEndpoint!.browseEndpoint!.browseId}`, name: artistRun.text } : undefined;
  const fill = (list: Track[]) =>
    list.map((t) => ({ ...t, artwork: t.artwork ?? artwork, artists: t.artists.length ? t.artists : artist ? [artist] : [] }));
  const tracks = fill(parseSearch(response));
  for (let page = 0; page < 20 && tracks.length < limit; page++) {
    const token = ([...findAll(response, "continuationCommand")][0] as { token?: string } | undefined)?.token
      ?? ([...findAll(response, "nextContinuationData")][0] as { continuation?: string } | undefined)?.continuation;
    if (!token) break;
    response = await call("browse", { continuation: token }, signal);
    const more = fill(parseSearch(response)).filter((t) => !tracks.some((x) => x.id === t.id));
    if (!more.length) break;
    tracks.push(...more);
  }
  return { title, subtitle, artwork, artist, tracks: tracks.slice(0, limit) };
}

/** tracks of a YouTube / YouTube Music playlist */
export async function youTubePlaylist(playlistId: string, signal?: AbortSignal, limit = 500): Promise<{ title: string; tracks: Track[] }> {
  const { title, tracks } = await youTubeCollection(playlistId.startsWith("VL") ? playlistId : `VL${playlistId}`, signal, limit);
  return { title, tracks };
}

/** a "card" item (carousels on artist pages): a video or song with its views */
export function parseTwoRowItem(item: Json): Track | null {
  const watch = (item.navigationEndpoint as Run["navigationEndpoint"] | undefined)?.watchEndpoint;
  const videoId = watch?.videoId;
  const title = (item.title as { runs?: Run[] } | undefined)?.runs?.map((r) => r.text).join("");
  if (!videoId || !title) return null;
  const sub = (item.subtitle as { runs?: Run[] } | undefined)?.runs ?? [];
  const artists = sub
    .filter((r) => r.navigationEndpoint?.browseEndpoint?.browseId)
    .map((r) => ({ id: `yta:${r.navigationEndpoint!.browseEndpoint!.browseId}`, name: r.text }));
  const views = sub.map((r) => r.text).find((t) => /\d/.test(t) && /(просм|views|прослуш|plays)/i.test(t));
  const thumbs = [...findAll(item.thumbnailRenderer, "thumbnails")][0] as unknown as Array<{ url: string }> | undefined;
  return {
    id: `yt:${videoId}`,
    source: "youtube",
    title,
    artists,
    durationMs: 0,
    artwork: thumbs?.[thumbs.length - 1]?.url,
    plays: views ? parseCount(views) : undefined,
    permalink: `https://music.youtube.com/watch?v=${videoId}`,
  };
}

export interface YouTubeArtist {
  name: string;
  avatar?: string;
  monthlyListeners?: number;
  subscribers?: number;
  tracks: Track[];
}

export function parseArtist(response: unknown): YouTubeArtist {
  const header = ([...findAll(response, "musicImmersiveHeaderRenderer")][0] ?? [...findAll(response, "musicVisualHeaderRenderer")][0] ?? {}) as Json;
  const name = (header.title as { runs?: Run[] } | undefined)?.runs?.[0]?.text ?? "YouTube";
  const monthly = (header.monthlyListenerCount as { runs?: Run[] } | undefined)?.runs?.[0]?.text;
  const subs = ([...findAll(header, "subscriberCountText")][0] as { runs?: Run[] } | undefined)?.runs?.[0]?.text;
  const thumbs = [...findAll(header.thumbnail, "thumbnails")][0] as unknown as Array<{ url: string }> | undefined;
  const tracks = parseSearch(response);
  const seen = new Set(tracks.map((t) => t.id));
  for (const item of findAll(response, "musicTwoRowItemRenderer")) {
    const track = parseTwoRowItem(item);
    if (track && !seen.has(track.id)) {
      seen.add(track.id);
      tracks.push(track);
    }
  }
  return {
    name,
    avatar: thumbs?.[thumbs.length - 1]?.url,
    monthlyListeners: monthly ? parseCount(monthly) : undefined,
    subscribers: subs ? parseCount(subs) : undefined,
    tracks,
  };
}

export async function youTubeArtist(channelId: string, signal?: AbortSignal): Promise<YouTubeArtist> {
  return parseArtist(await call("browse", { browseId: channelId.replace(/^yta:/, "") }, signal));
}

/** one song in a YouTube Music "up next" list */
export function parsePanelItem(item: Json): Track | null {
  const videoId = item.videoId as string | undefined;
  const title = (item.title as { runs?: Run[] } | undefined)?.runs?.map((r) => r.text).join("");
  if (!videoId || !title) return null;
  const byline = (item.longBylineText as { runs?: Run[] } | undefined)?.runs ?? [];
  const artists = byline
    .filter((r) => r.navigationEndpoint?.browseEndpoint?.browseEndpointContextSupportedConfigs?.browseEndpointContextMusicConfig?.pageType?.match(/ARTIST|USER_CHANNEL/))
    .map((r) => ({ id: `yta:${r.navigationEndpoint!.browseEndpoint!.browseId}`, name: r.text }));
  if (!artists.length && byline[0]?.text) artists.push({ id: `ytn:${byline[0].text}`, name: byline[0].text });
  const views = byline.map((r) => r.text).find((t) => /\d/.test(t) && /(просм|views|прослуш|plays)/i.test(t));
  const length = (item.lengthText as { runs?: Run[] } | undefined)?.runs?.[0]?.text ?? "";
  return {
    id: `yt:${videoId}`,
    source: "youtube",
    title,
    artists,
    durationMs: parseDuration(length),
    artwork: bestThumb(item),
    plays: views ? parseCount(views) : undefined,
    permalink: `https://music.youtube.com/watch?v=${videoId}`,
  };
}

export function parseUpNext(response: unknown): Track[] {
  const out: Track[] = [];
  for (const item of findAll(response, "playlistPanelVideoRenderer")) {
    const t = parsePanelItem(item);
    if (t && !out.some((x) => x.id === t.id)) out.push(t);
  }
  return out;
}

/** YouTube Music's own radio for a song: what plays after it on music.youtube.com */
export async function youTubeRadio(videoId: string, signal?: AbortSignal): Promise<Track[]> {
  const id = videoId.replace(/^yt:/, "");
  const response = await call("next", { videoId: id, playlistId: `RDAMVM${id}`, isAudioOnly: true }, signal);
  return parseUpNext(response).filter((t) => t.id !== `yt:${id}`);
}
