import { useSettings } from "@/settings/store";
import type { NamedTrack } from "@/library/matchTracks";

// Spotify's official Web API, with an app of your own (developer.spotify.com, free).
// Without it the import reads Spotify's embed page, which only lists the first 100 songs.

let token: { value: string; until: number; key: string } | null = null;

export function hasSpotifyApp(): boolean {
  const s = useSettings.getState();
  return Boolean(s.spotifyClientId.trim() && s.spotifyClientSecret.trim());
}

async function accessToken(signal?: AbortSignal): Promise<string> {
  const s = useSettings.getState();
  const id = s.spotifyClientId.trim();
  const secret = s.spotifyClientSecret.trim();
  const key = `${id}:${secret}`;
  if (token && token.key === key && Date.now() < token.until) return token.value;
  const res = await fetch("https://accounts.spotify.com/api/token", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded", authorization: `Basic ${btoa(key)}` },
    body: "grant_type=client_credentials",
    signal,
  });
  if (!res.ok) throw new Error("Spotify не принял Client ID / Client Secret — проверьте их в настройках");
  const body = (await res.json()) as { access_token: string; expires_in: number };
  token = { value: body.access_token, until: Date.now() + (body.expires_in - 60) * 1000, key };
  return token.value;
}

async function get<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { headers: { authorization: `Bearer ${await accessToken(signal)}` }, signal });
  if (res.status === 404) throw new Error("Spotify не отдаёт этот плейлист сторонним приложениям (так бывает с плейлистами, которые собирает сам Spotify)");
  if (!res.ok) throw new Error(`Spotify ответил ${res.status}`);
  return (await res.json()) as T;
}

interface ApiTrack {
  name: string;
  duration_ms: number;
  artists: Array<{ name: string }>;
  type?: string;
}

const named = (t: ApiTrack | null | undefined): NamedTrack[] =>
  t && t.name && t.type !== "episode" ? [{ title: t.name, artists: t.artists.map((a) => a.name), durationMs: t.duration_ms }] : [];

/** every song of a playlist, album or track */
export async function spotifyCollection(kind: "playlist" | "album" | "track", id: string, signal?: AbortSignal): Promise<{ title: string; tracks: NamedTrack[] }> {
  const api = "https://api.spotify.com/v1";
  if (kind === "track") {
    const t = await get<ApiTrack>(`${api}/tracks/${id}`, signal);
    return { title: t.name, tracks: named(t) };
  }
  if (kind === "album") {
    const album = await get<{ name: string; tracks: { items: ApiTrack[]; next: string | null } }>(`${api}/albums/${id}`, signal);
    const tracks = album.tracks.items.flatMap(named);
    let next = album.tracks.next;
    while (next) {
      const page = await get<{ items: ApiTrack[]; next: string | null }>(next, signal);
      tracks.push(...page.items.flatMap(named));
      next = page.next;
    }
    return { title: album.name, tracks };
  }
  const meta = await get<{ name: string }>(`${api}/playlists/${id}?fields=name`, signal);
  const tracks: NamedTrack[] = [];
  let next: string | null = `${api}/playlists/${id}/tracks?limit=100&fields=next,items(track(name,duration_ms,type,artists(name)))`;
  while (next && tracks.length < 10_000) {
    const page: { items: Array<{ track: ApiTrack | null }>; next: string | null } = await get(next, signal);
    tracks.push(...page.items.flatMap((i) => named(i.track)));
    next = page.next;
  }
  return { title: meta.name, tracks };
}
