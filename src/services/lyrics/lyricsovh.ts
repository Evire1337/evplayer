import type { LyricsHint } from "./genius";

// Second fallback for plain (unsynced) lyrics: lyrics.ovh, a free open service that needs no
// key or account. It looks a song up by exactly this artist and this title, so it only
// answers when the names are spelled the way it has them; that is why it comes after Genius.

export const LYRICS_OVH_PROVIDER = "lyrics.ovh";

/** the lyrics out of the service's answer; its own header line and empty answers are dropped */
export function cleanOvhLyrics(raw: string | undefined): string | undefined {
  if (!raw) return undefined;
  const lines = raw
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .map((line) => line.trimEnd());
  // some answers start with "Paroles de la chanson ... par ..."
  if (lines[0] && /^(paroles de la chanson|lyrics for)\b/i.test(lines[0].trim())) lines.shift();
  const text = lines.join("\n").replace(/\n{3,}/g, "\n\n").trim();
  return text.length > 20 ? text : undefined;
}

export async function findOvhLyrics(hint: LyricsHint, signal?: AbortSignal): Promise<string | undefined> {
  const artist = hint.artist.trim();
  const title = hint.title.trim();
  if (!artist || !title) return undefined;
  const res = await fetch(`https://api.lyrics.ovh/v1/${encodeURIComponent(artist)}/${encodeURIComponent(title)}`, { signal });
  if (!res.ok) return undefined;
  const body = (await res.json()) as { lyrics?: string };
  return cleanOvhLyrics(body.lyrics);
}
