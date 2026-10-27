import type { Track } from "@/types";
import { resolveSoundCloudUrl } from "@/services/soundcloud/api";
import { youTubePlaylist } from "@/services/youtube/music";
import { decodePlaylistCode, looksLikePlaylistCode } from "./playlistCode";
import { matchTracks, type NamedTrack } from "./matchTracks";
import { useLibrary } from "./store";
import { useImportProgress } from "./importProgress";
import { toast } from "@/ui/toasts";
import { hasSpotifyApp, spotifyCollection } from "@/services/spotify/webApi";
import { useNavigation } from "@/ui/navigation";

// "Import by link": paste a playlist, album or track link from a service and it becomes
// one of your playlists.

export type LinkKind = "soundcloud" | "youtube" | "spotify" | "deezer" | "code" | "apple" | null;

export function linkKind(text: string): LinkKind {
  const value = text.trim();
  if (looksLikePlaylistCode(value)) return "code";
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return null;
  }
  const host = url.hostname.replace(/^www\./, "").replace(/^m\./, "");
  if (host === "soundcloud.com" || host === "on.soundcloud.com") return "soundcloud";
  if (host === "youtube.com" || host === "music.youtube.com" || host === "youtu.be") return "youtube";
  if (host === "open.spotify.com" || host === "spotify.link") return "spotify";
  if (host === "deezer.com" || host === "link.deezer.com") return "deezer";
  if (host === "music.apple.com") return "apple";
  return null;
}

export const looksLikeLink = (text: string) => linkKind(text) !== null;

interface Fetched {
  title: string;
  description?: string;
  tracks?: Track[];
  named?: NamedTrack[];
  /** the source showed only part of the list */
  truncated?: boolean;
}

async function youtube(url: URL, signal: AbortSignal): Promise<Fetched> {
  const list = url.searchParams.get("list");
  if (list) {
    const { title, tracks } = await youTubePlaylist(list, signal);
    return { title, tracks, description: "Импортировано из YouTube" };
  }
  const id = url.hostname === "youtu.be" ? url.pathname.slice(1) : url.searchParams.get("v");
  if (!id) throw new Error("В ссылке нет видео или плейлиста");
  const res = await fetch(`https://www.youtube.com/oembed?format=json&url=${encodeURIComponent(`https://www.youtube.com/watch?v=${id}`)}`, { signal });
  const info = res.ok ? ((await res.json()) as { title?: string; author_name?: string; thumbnail_url?: string }) : {};
  const track: Track = {
    id: `yt:${id}`,
    source: "youtube",
    title: info.title ?? "YouTube",
    artists: info.author_name ? [{ id: `ytn:${info.author_name}`, name: info.author_name }] : [],
    durationMs: 0,
    artwork: info.thumbnail_url,
    permalink: `https://music.youtube.com/watch?v=${id}`,
  };
  return { title: track.title, tracks: [track] };
}

/** Spotify's embed page (the one other sites use for the Spotify player) lists the songs */
async function spotify(url: URL, signal: AbortSignal): Promise<Fetched> {
  let target = url;
  if (url.hostname === "spotify.link") {
    const res = await fetch(url, { signal });
    target = new URL(res.url);
  }
  const m = /\/(playlist|album|track)\/([A-Za-z0-9]+)/.exec(target.pathname);
  if (!m) throw new Error("Ссылка Spotify должна вести на плейлист, альбом или трек");
  if (hasSpotifyApp()) {
    // the whole list through the official API
    const full = await spotifyCollection(m[1] as "playlist" | "album" | "track", m[2], signal);
    return { title: full.title, named: full.tracks, description: "Импортировано из Spotify" };
  }
  const res = await fetch(`https://open.spotify.com/embed/${m[1]}/${m[2]}`, { signal });
  if (!res.ok) throw new Error(`Spotify ответил ${res.status}`);
  const html = await res.text();
  const json = /<script id="__NEXT_DATA__" type="application\/json">(.+?)<\/script>/s.exec(html)?.[1];
  if (!json) throw new Error("Spotify не показал список треков");
  const entity = JSON.parse(json)?.props?.pageProps?.state?.data?.entity as
    | { name?: string; title?: string; subtitle?: string; duration?: number; trackList?: Array<{ title: string; subtitle: string; duration?: number }> }
    | undefined;
  if (!entity) throw new Error("Spotify не показал список треков");
  const list = entity.trackList?.length
    ? entity.trackList
    : [{ title: entity.name ?? entity.title ?? "", subtitle: entity.subtitle ?? "", duration: entity.duration }];
  const named = list
    .filter((t) => t.title)
    .map((t) => ({ title: t.title, artists: t.subtitle.split(/,\s*/).filter(Boolean), durationMs: t.duration }));
  // the embed page stops at 100 songs: a longer playlist needs the official API
  const cut = m[1] === "playlist" && list.length >= 100;
  return { title: entity.name ?? entity.title ?? "Spotify", named, description: "Импортировано из Spotify", truncated: cut };
}

async function deezer(url: URL, signal: AbortSignal): Promise<Fetched> {
  let target = url;
  if (url.hostname === "link.deezer.com") {
    const res = await fetch(url, { signal });
    target = new URL(res.url);
  }
  const m = /\/(playlist|album|track)\/(\d+)/.exec(target.pathname);
  if (!m) throw new Error("Ссылка Deezer должна вести на плейлист, альбом или трек");
  const res = await fetch(`https://api.deezer.com/${m[1]}/${m[2]}`, { signal });
  const body = (await res.json()) as {
    title?: string;
    error?: { message?: string };
    duration?: number;
    artist?: { name: string };
    tracks?: { data?: Array<{ title: string; duration: number; artist?: { name: string } }> };
  };
  if (body.error) throw new Error(body.error.message ?? "Deezer не нашёл ссылку");
  const rows = m[1] === "track" ? [{ title: body.title ?? "", duration: body.duration ?? 0, artist: body.artist }] : (body.tracks?.data ?? []);
  return {
    title: body.title ?? "Deezer",
    description: "Импортировано из Deezer",
    named: rows.map((t) => ({ title: t.title, artists: t.artist ? [t.artist.name] : [], durationMs: t.duration * 1000 })),
  };
}

async function fetchLink(text: string, signal: AbortSignal): Promise<Fetched> {
  const kind = linkKind(text);
  if (kind === "code") {
    const shared = await decodePlaylistCode(text.trim());
    return { title: shared.title, description: shared.description, tracks: shared.tracks };
  }
  const url = new URL(text.trim());
  switch (kind) {
    case "soundcloud": {
      const { title, tracks } = await resolveSoundCloudUrl(url.toString(), signal);
      return { title, tracks, description: "Импортировано из SoundCloud" };
    }
    case "youtube":
      return youtube(url, signal);
    case "spotify":
      return spotify(url, signal);
    case "deezer":
      return deezer(url, signal);
    case "apple":
      throw new Error("Apple Music не даёт читать плейлисты без своего аккаунта разработчика. Экспортируйте плейлист в Spotify или вставьте названия вручную.");
    default:
      throw new Error("Эту ссылку не получилось распознать");
  }
}

let running: AbortController | null = null;

/** fetch the link, match songs if needed, save as a new playlist and open it */
export async function importFromLink(text: string): Promise<void> {
  running?.abort();
  const controller = new AbortController();
  running = controller;
  const progress = useImportProgress.getState();
  progress.start("Читаю ссылку…", () => controller.abort());
  try {
    const fetched = await fetchLink(text, controller.signal);
    let tracks = fetched.tracks ?? [];
    if (fetched.named?.length) {
      progress.update(`Ищу песни: 0 из ${fetched.named.length}`);
      tracks = await matchTracks(fetched.named, {
        signal: controller.signal,
        onProgress: (p) => progress.update(`Ищу песни: ${p.done} из ${p.total} (найдено ${p.found})`),
      });
    }
    if (controller.signal.aborted) return;
    if (!tracks.length) throw new Error("В ссылке не нашлось треков, которые можно слушать");
    const lib = useLibrary.getState();
    const playlist = lib.createPlaylist(fetched.title, fetched.description);
    lib.addToPlaylist(playlist.id, tracks);
    const missed = fetched.named ? fetched.named.length - tracks.length : 0;
    toast(missed > 0 ? `Плейлист «${fetched.title}»: ${tracks.length} треков, ${missed} не нашлось` : `Плейлист «${fetched.title}» добавлен`);
    if (fetched.truncated) {
      toast("Spotify без своего приложения показывает только первые 100 треков. Подключите Spotify в Настройки → Сервисы — и импорт заберёт плейлист целиком", "error");
    }
    useNavigation.getState().go("playlist", playlist.id);
  } catch (error) {
    if (!controller.signal.aborted) toast(error instanceof Error ? error.message : String(error), "error");
  } finally {
    if (running === controller) running = null;
    progress.finish();
  }
}
