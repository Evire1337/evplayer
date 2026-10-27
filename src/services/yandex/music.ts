import type { NamedTrack } from "@/library/matchTracks";

// Yandex Music: public playlists, albums and tracks, and the public side of a profile.
// Only names are read; the songs are then found on YouTube Music and SoundCloud. Yandex
// Music is not available in every country — then its server answers with an error.

const API = "https://api.music.yandex.net";

interface YmArtist {
  name?: string;
}
export interface YmTrack {
  id?: string | number;
  title?: string;
  version?: string;
  durationMs?: number;
  artists?: YmArtist[];
  available?: boolean;
}

export type YandexLink =
  | { kind: "playlist"; owner: string; id: string }
  | { kind: "playlist-uuid"; id: string }
  | { kind: "album"; id: string }
  | { kind: "track"; id: string }
  | { kind: "user"; owner: string };

const HOSTS = /^(music\.yandex\.(ru|com|by|kz|uz)|next\.music\.yandex\.ru)$/;

export function isYandexMusicUrl(url: URL): boolean {
  return HOSTS.test(url.hostname.replace(/^www\./, ""));
}

export function parseYandexLink(text: string): YandexLink | null {
  let url: URL;
  try {
    url = new URL(text.trim());
  } catch {
    return null;
  }
  if (!isYandexMusicUrl(url)) return null;
  const path = url.pathname.replace(/\/+$/, "");
  let m = /\/users\/([^/]+)\/playlists\/(\d+)/.exec(path);
  if (m) return { kind: "playlist", owner: decodeURIComponent(m[1]), id: m[2] };
  m = /\/playlists\/((?:lk\.)?[0-9a-f-]{20,})/i.exec(path);
  if (m) return { kind: "playlist-uuid", id: m[1] };
  m = /\/album\/\d+\/track\/(\d+)/.exec(path);
  if (m) return { kind: "track", id: m[1] };
  m = /\/track\/(\d+)/.exec(path);
  if (m) return { kind: "track", id: m[1] };
  m = /\/album\/(\d+)/.exec(path);
  if (m) return { kind: "album", id: m[1] };
  m = /\/users\/([^/]+)/.exec(path);
  if (m) return { kind: "user", owner: decodeURIComponent(m[1]) };
  return null;
}

export function namedFromYandex(track: YmTrack | undefined | null): NamedTrack | null {
  if (!track?.title) return null;
  const title = track.version ? `${track.title} (${track.version})` : track.title;
  return { title, artists: (track.artists ?? []).map((a) => a.name ?? "").filter(Boolean), durationMs: track.durationMs };
}

async function get<T>(path: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(`${API}${path}`, { signal });
  const body = (await res.json().catch(() => ({}))) as { result?: T; error?: { name?: string; message?: string } };
  if (res.status === 451 || body.error?.name === "Unavailable For Legal Reasons") {
    throw new Error("Яндекс Музыка недоступна в вашей стране — нужен доступ из региона, где она работает");
  }
  if (res.status === 404 || body.error?.name === "not-found") throw new Error("Яндекс Музыка не нашла это — проверьте ссылку");
  if (res.status === 403 || res.status === 401) throw new Error("Это закрыто: сделайте плейлист или профиль публичным в Яндекс Музыке");
  if (!res.ok || !body.result) throw new Error(`Яндекс Музыка ответила ${res.status}`);
  return body.result;
}

function fromPlaylist(result: { title?: string; tracks?: Array<{ track?: YmTrack }> }): { title: string; tracks: NamedTrack[] } {
  return {
    title: result.title ?? "Яндекс Музыка",
    tracks: (result.tracks ?? []).flatMap((t) => namedFromYandex(t.track) ?? []),
  };
}

/** the songs behind a Yandex Music link */
export async function yandexCollection(link: YandexLink, signal?: AbortSignal): Promise<{ title: string; tracks: NamedTrack[] }> {
  switch (link.kind) {
    case "playlist":
      return fromPlaylist(await get(`/users/${encodeURIComponent(link.owner)}/playlists/${link.id}`, signal));
    case "playlist-uuid":
      return fromPlaylist(await get(`/playlist/${link.id}`, signal));
    case "album": {
      const album = await get<{ title?: string; volumes?: YmTrack[][] }>(`/albums/${link.id}/with-tracks`, signal);
      return { title: album.title ?? "Яндекс Музыка", tracks: (album.volumes ?? []).flat().flatMap((t) => namedFromYandex(t) ?? []) };
    }
    case "track": {
      const list = await get<YmTrack[]>(`/tracks/${link.id}`, signal);
      const named = namedFromYandex(list[0]);
      return { title: named?.title ?? "Яндекс Музыка", tracks: named ? [named] : [] };
    }
    case "user": {
      const likes = await yandexLikes(link.owner, signal);
      return { title: `Мне нравится · ${link.owner}`, tracks: likes };
    }
  }
}

export interface YandexProfile {
  login: string;
  name: string;
  playlists: Array<{ id: string; title: string; count: number; cover?: string }>;
}

/** a profile's public playlists ("connect the account" by login) */
export async function yandexProfile(login: string, signal?: AbortSignal): Promise<YandexProfile> {
  const clean = login.trim().replace(/^@/, "");
  const list = await get<Array<{ kind: number; title: string; trackCount?: number; owner?: { login?: string; name?: string }; cover?: { uri?: string } }>>(
    `/users/${encodeURIComponent(clean)}/playlists/list`,
    signal,
  );
  return {
    login: clean,
    name: list[0]?.owner?.name ?? clean,
    playlists: list.map((p) => ({
      id: String(p.kind),
      title: p.title,
      count: p.trackCount ?? 0,
      cover: p.cover?.uri ? `https://${p.cover.uri.replace("%%", "400x400")}` : undefined,
    })),
  };
}

/** "Мне нравится" of a profile, if it is public */
export async function yandexLikes(login: string, signal?: AbortSignal): Promise<NamedTrack[]> {
  const likes = await get<{ library?: { tracks?: Array<{ id: string | number; albumId?: string | number }> } }>(
    `/users/${encodeURIComponent(login)}/likes/tracks`,
    signal,
  );
  const ids = (likes.library?.tracks ?? []).map((t) => (t.albumId ? `${t.id}:${t.albumId}` : String(t.id)));
  const out: NamedTrack[] = [];
  for (let i = 0; i < ids.length; i += 200) {
    const res = await fetch(`${API}/tracks`, {
      method: "POST",
      headers: { "content-type": "application/x-www-form-urlencoded" },
      body: `track-ids=${encodeURIComponent(ids.slice(i, i + 200).join(","))}`,
      signal,
    });
    const body = (await res.json().catch(() => ({}))) as { result?: YmTrack[] };
    out.push(...(body.result ?? []).flatMap((t) => namedFromYandex(t) ?? []));
  }
  return out;
}
