import type { ArtistSummary, CollectionSummary, Track } from "@/types";
import { useSettings } from "@/settings/store";
import { mapScCollection, mapScTrack, mapScUser, rankTranscodings, trackAuthorization, transcodingsOf } from "./mappers";

const API = "https://api-v2.soundcloud.com";

let clientId: string | null = null;

async function getClientId(force = false): Promise<string> {
  // a client id of your own registered SoundCloud app, if one is set in the settings
  const own = useSettings.getState().soundcloudClientId.trim();
  if (own) return own;
  if (clientId && !force) return clientId;
  const bridge = window.evplayer;
  if (!bridge) throw new Error("SoundCloud works in the desktop app only");
  clientId = await bridge.soundcloudClientId(force);
  return clientId;
}

/** GET from api-v2; a rejected client id is refreshed once */
export async function scGet<T = unknown>(path: string, params: Record<string, string> = {}, signal?: AbortSignal): Promise<T> {
  for (let attempt = 0; attempt < 2; attempt++) {
    const url = new URL(path.startsWith("http") ? path : `${API}${path}`);
    for (const [k, v] of Object.entries(params)) url.searchParams.set(k, v);
    url.searchParams.set("client_id", await getClientId(attempt > 0));
    const res = await fetch(url, { signal });
    if (res.status === 401 || res.status === 403) continue;
    if (!res.ok) throw new Error(`SoundCloud ответил ${res.status}`);
    return (await res.json()) as T;
  }
  throw new Error("SoundCloud отклонил запрос");
}

interface Collection {
  collection?: unknown[];
}

export async function searchSoundCloud(query: string, signal?: AbortSignal): Promise<{ tracks: Track[]; artists: ArtistSummary[] }> {
  const q = query.trim();
  if (!q) return { tracks: [], artists: [] };
  const [tracks, users] = await Promise.all([
    scGet<Collection>("/search/tracks", { q, limit: "30" }, signal),
    scGet<Collection>("/search/users", { q, limit: "6" }, signal).catch(() => ({ collection: [] })),
  ]);
  return {
    tracks: (tracks.collection ?? []).flatMap((t) => mapScTrack(t) ?? []),
    artists: (users.collection ?? []).flatMap((u) => mapScUser(u) ?? []),
  };
}

export async function soundCloudCharts(kind: "top" | "trending", signal?: AbortSignal): Promise<Track[]> {
  const body = await scGet<Collection>("/charts", { kind, genre: "soundcloud:genres:all-music", limit: "20" }, signal);
  return (body.collection ?? []).flatMap((row) => {
    const track = typeof row === "object" && row !== null ? (row as { track?: unknown }).track : undefined;
    return mapScTrack(track) ?? [];
  });
}

export interface ScStream {
  url: string;
  hls: boolean;
}

/** the reason a SoundCloud song has nothing this player can use */
export class ScUnplayableError extends Error {
  constructor() {
    super("SoundCloud отдаёт этот трек только в защищённом виде или 30-секундным отрывком");
  }
}

/**
 * Every format of a song this player can play, best first, each asked from SoundCloud only
 * when the previous one did not work out. A format that SoundCloud refuses is skipped.
 */
export async function* soundCloudStreams(trackId: string, signal?: AbortSignal): AsyncGenerator<ScStream> {
  const id = trackId.replace(/^sc:/, "");
  const track = await scGet(`/tracks/${id}`, {}, signal);
  const auth = trackAuthorization(track);
  const options = rankTranscodings(transcodingsOf(track));
  if (options.length === 0) throw new ScUnplayableError();
  let given = 0;
  let lastError: unknown;
  for (const option of options) {
    let url: string | undefined;
    try {
      ({ url } = await scGet<{ url?: string }>(option.url, auth ? { track_authorization: auth } : {}, signal));
    } catch (error) {
      if (signal?.aborted) throw error;
      lastError = error;
      continue;
    }
    if (url) {
      given++;
      yield { url, hls: option.protocol === "hls" };
    }
  }
  if (!given) throw lastError instanceof Error ? lastError : new Error("SoundCloud не отдал поток");
}

/** the first format of a song SoundCloud hands out */
export async function resolveSoundCloudStream(trackId: string, signal?: AbortSignal): Promise<ScStream> {
  for await (const stream of soundCloudStreams(trackId, signal)) return stream;
  throw new Error("SoundCloud не отдал поток");
}

/** songs SoundCloud considers similar: used for autoplay when the queue runs out */
export async function soundCloudRelated(trackId: string, signal?: AbortSignal): Promise<Track[]> {
  const id = trackId.replace(/^sc:/, "");
  const body = await scGet<Collection>(`/tracks/${id}/related`, { limit: "25" }, signal);
  return (body.collection ?? []).flatMap((t) => mapScTrack(t) ?? []);
}

export async function soundCloudUser(userId: string, signal?: AbortSignal): Promise<{ artist: ArtistSummary | null; tracks: Track[]; popular: Track[] }> {
  const id = userId.replace(/^scu?:/, "");
  const [user, tracks, popular] = await Promise.all([
    scGet<unknown>(`/users/${id}`, {}, signal),
    scGet<Collection>(`/users/${id}/tracks`, { limit: "50" }, signal),
    scGet<Collection>(`/users/${id}/toptracks`, { limit: "10" }, signal).catch(() => ({ collection: [] })),
  ]);
  return {
    artist: mapScUser(user),
    tracks: (tracks.collection ?? []).flatMap((t) => mapScTrack(t) ?? []),
    popular: (popular.collection ?? []).flatMap((t) => mapScTrack(t) ?? []),
  };
}

/** full track objects for ids (playlists only carry ids for most of their tracks) */
async function tracksByIds(ids: number[], signal?: AbortSignal): Promise<Track[]> {
  const out: Track[] = [];
  for (let i = 0; i < ids.length; i += 50) {
    const chunk = ids.slice(i, i + 50);
    const body = await scGet<unknown[]>("/tracks", { ids: chunk.join(",") }, signal);
    const byId = new Map((Array.isArray(body) ? body : []).flatMap((t) => {
      const track = mapScTrack(t);
      return track ? [[track.id, track] as const] : [];
    }));
    for (const id of chunk) {
      const track = byId.get(`sc:${id}`);
      if (track) out.push(track);
    }
  }
  return out;
}

/** a soundcloud.com link: a track, a playlist/album or a profile */
export async function resolveSoundCloudUrl(url: string, signal?: AbortSignal): Promise<{ title: string; tracks: Track[]; artwork?: string }> {
  const body = await scGet<Record<string, unknown>>("/resolve", { url }, signal);
  if (body.kind === "track") {
    const track = mapScTrack(body);
    return { title: track?.title ?? "SoundCloud", tracks: track ? [track] : [] };
  }
  if (body.kind === "playlist" || body.kind === "system-playlist") {
    const raw = Array.isArray(body.tracks) ? (body.tracks as Array<Record<string, unknown>>) : [];
    const ids = raw.map((t) => Number(t.id)).filter(Number.isFinite);
    const tracks = await tracksByIds(ids, signal);
    return { title: String(body.title ?? "SoundCloud"), tracks, artwork: mapScCollection(body)?.artwork ?? tracks.find((t) => t.artwork)?.artwork };
  }
  if (body.kind === "user") {
    const user = await soundCloudUser(String(body.id), signal);
    return { title: user.artist?.name ?? "SoundCloud", tracks: user.tracks };
  }
  throw new Error("Ссылка SoundCloud не похожа на трек или плейлист");
}

export async function searchSoundCloudCollections(query: string, kind: "album" | "playlist", signal?: AbortSignal): Promise<CollectionSummary[]> {
  const body = await scGet<Collection>(kind === "album" ? "/search/albums" : "/search/playlists_without_albums", { q: query, limit: "20" }, signal);
  return (body.collection ?? []).flatMap((c) => mapScCollection(c) ?? []);
}

export async function soundCloudCollection(id: string, signal?: AbortSignal): Promise<{ summary: CollectionSummary | null; tracks: Track[] }> {
  const body = await scGet<Record<string, unknown>>(`/playlists/${id.replace(/^scp:/, "")}`, {}, signal);
  const raw = Array.isArray(body.tracks) ? (body.tracks as Array<Record<string, unknown>>) : [];
  const ids = raw.map((t) => Number(t.id)).filter(Number.isFinite);
  return { summary: mapScCollection(body), tracks: await tracksByIds(ids, signal) };
}

/** public likes and playlists of a SoundCloud profile ("connect your account") */
export async function soundCloudUserLikes(userId: string, signal?: AbortSignal, limit = 1000): Promise<Track[]> {
  const id = userId.replace(/^scu?:/, "");
  const out: Track[] = [];
  let next: string | null = `/users/${id}/track_likes`;
  let params: Record<string, string> = { limit: "200" };
  while (next && out.length < limit) {
    const body: { collection?: Array<{ track?: unknown }>; next_href?: string | null } = await scGet(next, params, signal);
    for (const row of body.collection ?? []) {
      const track = mapScTrack(row.track);
      if (track) out.push(track);
    }
    next = body.next_href ?? null;
    params = {};
  }
  return out.slice(0, limit);
}

export async function soundCloudUserPlaylists(userId: string, signal?: AbortSignal): Promise<CollectionSummary[]> {
  const id = userId.replace(/^scu?:/, "");
  const body = await scGet<Collection>(`/users/${id}/playlists_without_albums`, { limit: "50" }, signal).catch(() => scGet<Collection>(`/users/${id}/playlists`, { limit: "50" }, signal));
  return (body.collection ?? []).flatMap((c) => mapScCollection(c) ?? []);
}

export async function resolveSoundCloudUser(input: string, signal?: AbortSignal): Promise<ArtistSummary & { permalink: string; likes?: number; tracks?: number }> {
  const value = input.trim();
  const url = /^https?:\/\//.test(value) ? value : `https://soundcloud.com/${value.replace(/^@/, "")}`;
  const body = await scGet<Record<string, unknown>>("/resolve", { url }, signal);
  const user = mapScUser(body);
  if (!user) throw new Error("По этой ссылке нет профиля SoundCloud");
  return {
    ...user,
    permalink: String(body.permalink_url ?? url),
    likes: typeof body.likes_count === "number" ? body.likes_count : undefined,
    tracks: typeof body.track_count === "number" ? body.track_count : undefined,
  };
}
