import type { Track } from "@/types";
import { player } from "@/player/engine";
import { scGet } from "@/services/soundcloud/api";
import { mapScTrack } from "@/services/soundcloud/mappers";

// A song as a link: Discord's "Слушать" button leads to a page on GitHub Pages (Discord takes
// only https links), and that page opens evplayer://play?... in the app. The link carries
// what is needed to show and play the song (the id is enough to get its sound).

export const LISTEN_PAGE = "https://evire1337.github.io/evplayer/listen/";

const ID = /^(sc:\d+|yt:[\w-]{11})$/;

/** the page that opens this song in EvPlayer (none for a file on this computer) */
export function listenUrl(track: Track): string | undefined {
  if (!ID.test(track.id)) return undefined;
  const q = new URLSearchParams({ id: track.id, t: track.title.slice(0, 120), a: track.artists.map((a) => a.name).join(", ").slice(0, 80) });
  if (track.durationMs) q.set("d", String(Math.round(track.durationMs)));
  if (track.permalink?.startsWith("https://")) q.set("p", track.permalink);
  if (track.artwork?.startsWith("https://")) q.set("art", track.artwork);
  let url = `${LISTEN_PAGE}?${q}`;
  // Discord takes a button link of 512 characters at most: the cover goes first, then the page
  if (url.length > 512) {
    q.delete("art");
    url = `${LISTEN_PAGE}?${q}`;
  }
  if (url.length > 512) {
    q.delete("p");
    url = `${LISTEN_PAGE}?${q}`;
  }
  return url.length <= 512 ? url : undefined;
}

/** the song an evplayer://play?... link stands for, or null when it is not one */
export function trackFromLink(url: string): Track | null {
  const m = /^evplayer:\/\/play\/?\?(.*)$/i.exec(url.trim());
  if (!m) return null;
  const q = new URLSearchParams(m[1]);
  const id = q.get("id") ?? "";
  if (!ID.test(id)) return null;
  const https = (k: string) => (q.get(k)?.startsWith("https://") ? q.get(k)! : undefined);
  const artist = (q.get("a") ?? "").slice(0, 80);
  return {
    id,
    source: id.startsWith("sc:") ? "soundcloud" : "youtube",
    title: (q.get("t") ?? "").slice(0, 200) || "Без названия",
    artists: artist ? artist.split(", ").map((name) => ({ id: "unknown", name })) : [],
    durationMs: Math.max(0, Number(q.get("d")) || 0),
    artwork: https("art"),
    permalink: https("p"),
  };
}

/** play the song of a link; a SoundCloud song is read again from SoundCloud for its full details */
export async function playLink(url: string): Promise<boolean> {
  const track = trackFromLink(url);
  if (!track) return false;
  let fresh: Track | null = null;
  if (track.source === "soundcloud") {
    fresh = await scGet(`/tracks/${track.id.slice(3)}`).then(mapScTrack).catch(() => null);
  }
  await player.playQueue([fresh ?? track], 0, null);
  return true;
}
