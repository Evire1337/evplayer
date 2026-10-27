import { spotifyClientId, useSettings } from "@/settings/store";
import type { NamedTrack } from "@/library/matchTracks";

// Spotify's official Web API. Access comes from either
//  - "Sign in with Spotify" (PKCE, any account, Premium not needed), or
//  - an app of your own with Client ID + Secret (reads public playlists).
// Without either, link imports read Spotify's embed page, which lists only 100 songs.
// Every list is read page by page (offset) until the end, so length does not matter.

const API = "https://api.spotify.com/v1";
let appToken: { value: string; until: number; key: string } | null = null;

export function hasSpotifyAccess(): boolean {
  const s = useSettings.getState();
  return Boolean(s.spotifyAccount || (s.spotifyClientId.trim() && s.spotifyClientSecret.trim()));
}
/** kept for older callers */
export const hasSpotifyApp = hasSpotifyAccess;

async function userToken(): Promise<string | null> {
  const s = useSettings.getState();
  const account = s.spotifyAccount;
  if (!account) return null;
  if (Date.now() < account.expiresAt) return account.accessToken;
  const fresh = await window.evplayer?.spotify.refresh(spotifyClientId(s), account.refreshToken);
  if (!fresh) throw new Error("Вход в Spotify устарел — войдите заново в настройках");
  useSettings.getState().set({ spotifyAccount: { ...account, ...fresh } });
  return fresh.accessToken;
}

async function accessToken(signal?: AbortSignal): Promise<string> {
  const user = await userToken().catch((e) => {
    throw e instanceof Error ? e : new Error(String(e));
  });
  if (user) return user;
  const s = useSettings.getState();
  const id = s.spotifyClientId.trim();
  const secret = s.spotifyClientSecret.trim();
  if (!id || !secret) throw new Error("Spotify не подключён: войдите через Spotify в Настройки → Сервисы");
  const key = `${id}:${secret}`;
  if (appToken && appToken.key === key && Date.now() < appToken.until) return appToken.value;
  const res = await fetch("https://accounts.spotify.com/api/token", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded", authorization: `Basic ${btoa(key)}` },
    body: "grant_type=client_credentials",
    signal,
  });
  if (!res.ok) throw new Error("Spotify не принял Client ID / Client Secret — проверьте их в настройках");
  const body = (await res.json()) as { access_token: string; expires_in: number };
  appToken = { value: body.access_token, until: Date.now() + (body.expires_in - 60) * 1000, key };
  return appToken.value;
}

async function get<T>(url: string, signal?: AbortSignal): Promise<T> {
  for (let attempt = 0; ; attempt++) {
    const res = await fetch(url.startsWith("http") ? url : `${API}${url}`, { headers: { authorization: `Bearer ${await accessToken(signal)}` }, signal });
    // too many requests: wait as long as Spotify asks, then try again
    if (res.status === 429 && attempt < 5) {
      const wait = Number(res.headers.get("retry-after") ?? "2");
      await new Promise((r) => setTimeout(r, Math.min(30, wait) * 1000));
      continue;
    }
    if (res.status === 401 && attempt === 0 && useSettings.getState().spotifyAccount) {
      const a = useSettings.getState().spotifyAccount!;
      useSettings.getState().set({ spotifyAccount: { ...a, expiresAt: 0 } });
      continue;
    }
    if (res.status === 404) throw new Error("Spotify не отдаёт этот плейлист сторонним приложениям (так бывает с подборками, которые собирает сам Spotify)");
    if (res.status === 403) throw new Error("Spotify не пустил: аккаунт не добавлен в приложение EvPlayer на developer.spotify.com (раздел User Management)");
    if (!res.ok) throw new Error(`Spotify ответил ${res.status}`);
    return (await res.json()) as T;
  }
}

/** every page of a list: Spotify gives at most 50-100 items per request */
async function allPages<T>(first: string, signal?: AbortSignal, onPage?: (count: number, total: number) => void): Promise<T[]> {
  const out: T[] = [];
  let next: string | null = first;
  while (next && out.length < 20_000) {
    const page: { items: T[]; next: string | null; total?: number } = await get(next, signal);
    out.push(...page.items);
    onPage?.(out.length, page.total ?? out.length);
    next = page.next;
  }
  return out;
}

interface ApiTrack {
  name: string;
  duration_ms: number;
  artists: Array<{ name: string }>;
  type?: string;
  is_local?: boolean;
}

export const named = (t: ApiTrack | null | undefined): NamedTrack[] =>
  t && t.name && t.type !== "episode" ? [{ title: t.name, artists: t.artists.map((a) => a.name).filter(Boolean), durationMs: t.duration_ms }] : [];

/** every song of a playlist, album or track, whatever the length */
export async function spotifyCollection(
  kind: "playlist" | "album" | "track",
  id: string,
  signal?: AbortSignal,
  onPage?: (count: number, total: number) => void,
): Promise<{ title: string; tracks: NamedTrack[]; artwork?: string }> {
  if (kind === "track") {
    const t = await get<ApiTrack>(`/tracks/${id}`, signal);
    return { title: t.name, tracks: named(t) };
  }
  if (kind === "album") {
    const album = await get<{ name: string; images?: Array<{ url: string }> }>(`/albums/${id}`, signal);
    const items = await allPages<ApiTrack>(`/albums/${id}/tracks?limit=50`, signal, onPage);
    return { title: album.name, tracks: items.flatMap(named), artwork: album.images?.[0]?.url };
  }
  const meta = await get<{ name: string; images?: Array<{ url: string }> }>(`/playlists/${id}?fields=name,images`, signal);
  const items = await allPages<{ track: ApiTrack | null }>(
    `/playlists/${id}/tracks?limit=100&offset=0&fields=next,total,items(track(name,duration_ms,type,artists(name)))`,
    signal,
    onPage,
  );
  return { title: meta.name, tracks: items.flatMap((i) => named(i.track)), artwork: meta.images?.[0]?.url };
}

export async function spotifyMe(): Promise<{ name: string; avatar?: string; product?: string }> {
  const me = await get<{ display_name?: string; id: string; images?: Array<{ url: string }>; product?: string }>("/me");
  return { name: me.display_name || me.id, avatar: me.images?.[0]?.url, product: me.product };
}

/** "Liked Songs" of the signed-in account */
export async function spotifyLikedSongs(signal?: AbortSignal, onPage?: (count: number, total: number) => void): Promise<NamedTrack[]> {
  const items = await allPages<{ track: ApiTrack | null }>("/me/tracks?limit=50", signal, onPage);
  return items.flatMap((i) => named(i.track));
}

export async function spotifyMyPlaylists(signal?: AbortSignal): Promise<Array<{ id: string; name: string; total: number; artwork?: string }>> {
  const items = await allPages<{ id: string; name: string; tracks?: { total: number }; images?: Array<{ url: string }> | null }>("/me/playlists?limit=50", signal);
  return items.map((p) => ({ id: p.id, name: p.name, total: p.tracks?.total ?? 0, artwork: p.images?.[0]?.url }));
}
