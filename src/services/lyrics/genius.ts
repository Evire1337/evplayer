export interface LyricsHint {
  title: string;
  artist: string;
}

// Fallback for songs LRCLIB does not know: plain (unsynced) lyrics from genius.com, the
// same page a browser would show. Genius has no timing, so these lines do not follow along.

export const GENIUS_PROVIDER = "Genius";

interface GeniusHit {
  result?: {
    title?: string;
    url?: string;
    instrumental?: boolean;
    lyrics_state?: string;
    primary_artist?: { name?: string };
    artist_names?: string;
  };
}

function norm(value: string): string {
  return value
    .toLowerCase()
    .replace(/ё/g, "е")
    .replace(/[([{][^)\]}]*[)\]}]/g, " ")
    .replace(/\b(feat|ft|prod)\b\.?.*$/u, " ")
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();
}

function overlap(a: string, b: string): number {
  const left = new Set(a.split(" ").filter((w) => w.length > 1));
  const right = new Set(b.split(" ").filter((w) => w.length > 1));
  if (!left.size || !right.size) return 0;
  let shared = 0;
  for (const w of left) if (right.has(w)) shared++;
  return shared / Math.min(left.size, right.size);
}

export function pickGeniusHit(hits: GeniusHit[], hint: LyricsHint): GeniusHit["result"] | undefined {
  const title = norm(hint.title);
  const artist = norm(hint.artist);
  let best: GeniusHit["result"] | undefined;
  let bestScore = 0;
  for (const hit of hits) {
    const result = hit.result;
    if (!result?.url || result.instrumental || (result.lyrics_state && result.lyrics_state !== "complete")) continue;
    const gotTitle = norm(result.title ?? "");
    const gotArtist = norm(result.artist_names ?? result.primary_artist?.name ?? "");
    let score = 0;
    if (gotTitle === title) score += 60;
    else if (gotTitle && (title.includes(gotTitle) || gotTitle.includes(title))) score += 35;
    else if (overlap(title, gotTitle) >= 0.67) score += 20;
    else continue;
    if (artist && gotArtist) {
      if (gotArtist.includes(artist) || artist.includes(gotArtist) || overlap(artist, gotArtist) >= 0.5) score += 30;
      else score -= 25;
    }
    if (score > bestScore) {
      best = result;
      bestScore = score;
    }
  }
  return bestScore >= 35 ? best : undefined;
}

/** lyrics text out of a genius.com song page */
export function extractGeniusLyrics(html: string): string | undefined {
  const doc = new DOMParser().parseFromString(html, "text/html");
  const containers = [...doc.querySelectorAll('[data-lyrics-container="true"]')];
  if (containers.length === 0) return undefined;
  const parts: string[] = [];
  for (const container of containers) {
    container.querySelectorAll('[data-exclude-from-selection="true"]').forEach((el) => el.remove());
    container.querySelectorAll("br").forEach((br) => br.replaceWith("\n"));
    parts.push(container.textContent ?? "");
  }
  const lines = parts
    .join("\n")
    .split("\n")
    .map((line) => line.trim())
    // section labels like [Chorus] / [Припев] are not sung
    .filter((line) => !/^\[[^\]]*\]$/.test(line));
  const text = lines.join("\n").replace(/\n{3,}/g, "\n\n").trim();
  return text.length > 20 ? text : undefined;
}

export async function findGeniusLyrics(hint: LyricsHint, signal?: AbortSignal): Promise<string | undefined> {
  const query = `${hint.artist} ${hint.title}`.trim();
  if (!query) return undefined;
  const search = await fetch(`https://genius.com/api/search/song?per_page=5&q=${encodeURIComponent(query)}`, { signal });
  if (!search.ok) return undefined;
  const body = (await search.json()) as { response?: { sections?: Array<{ hits?: GeniusHit[] }> } };
  const hits = body.response?.sections?.flatMap((section) => section.hits ?? []) ?? [];
  const hit = pickGeniusHit(hits, hint);
  if (!hit?.url) return undefined;
  const page = await fetch(hit.url, { signal });
  if (!page.ok) return undefined;
  const lyrics = extractGeniusLyrics(await page.text());
  return lyrics;
}
