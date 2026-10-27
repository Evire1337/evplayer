import type { ArtistSummary, Track } from "@/types";
import { mapScTrack, mapScUser, pickTranscoding, transcodingsOf, type Transcoding } from "./mappers";

const API = "https://api-v2.soundcloud.com";

let clientId: string | null = null;

async function getClientId(force = false): Promise<string> {
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
    if (!res.ok) throw new Error(`SoundCloud answered ${res.status}`);
    return (await res.json()) as T;
  }
  throw new Error("SoundCloud refused the request");
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

export async function resolveSoundCloudStream(trackId: string, signal?: AbortSignal): Promise<ScStream> {
  const id = trackId.replace(/^sc:/, "");
  const track = await scGet(`/tracks/${id}`, {}, signal);
  const choice: Transcoding | undefined = pickTranscoding(transcodingsOf(track));
  if (!choice) throw new Error("This track cannot be streamed");
  const { url } = await scGet<{ url?: string }>(choice.url, {}, signal);
  if (!url) throw new Error("SoundCloud gave no stream address");
  return { url, hls: choice.protocol === "hls" };
}
