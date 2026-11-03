import type { NamedTrack } from "./matchTracks";

// A list of songs pasted as text: "Artist — Title" lines (numbered or with a length, as
// many sites and notes give them), or the rows copied from a VK playlist page, which come as
// three lines each: the title, the artist and the length. Each song is then looked up on
// YouTube Music and SoundCloud like any other imported name.
//
// VK does not let other apps read anyone's music (only its own apps may), so copying the
// list from the page is the way to bring a VK playlist or «Моя музыка» over.

const DURATION = /^(?:\d{1,2}:)?\d{1,2}:\d{2}$/;
const SEPARATOR = /\s+[-–—]\s+/;
/** row numbers, bullets, "explicit" marks and other bits around a name */
const LEAD = /^\s*(?:\d{1,4}[.)]\s*|[•·*▶►-]\s+)/;
const TAIL = /\s+(?:(?:\d{1,2}:)?\d{1,2}:\d{2}|E|🅴)\s*$/u;

function durationMs(text: string): number | undefined {
  const parts = text.split(":").map(Number);
  if (parts.some((n) => !Number.isFinite(n))) return undefined;
  return parts.reduce((total, n) => total * 60 + n, 0) * 1000;
}

function clean(line: string): string {
  let s = line.replace(LEAD, "").trim();
  // a length or a mark at the end of the line
  for (let i = 0; i < 2; i++) s = s.replace(TAIL, "").trim();
  return s;
}

/** the artists of "A, B feat. C" */
function artistsOf(text: string): string[] {
  return text
    .split(/\s*(?:,|&|\bfeat\.?|\bft\.?|\bx\b)\s*/i)
    .map((a) => a.trim())
    .filter(Boolean);
}

export function parseTrackList(text: string): NamedTrack[] {
  const lines = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean);
  const out: NamedTrack[] = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (DURATION.test(line)) continue;
    // "Artist — Title" (and a length after it, on the line or the next one)
    const tail = line.match(/((?:\d{1,2}:)?\d{1,2}:\d{2})\s*$/);
    const cleaned = clean(line);
    const split = cleaned.split(SEPARATOR);
    if (split.length >= 2 && split[0] && split[1]) {
      const next = lines[i + 1];
      const length = tail ? durationMs(tail[1]) : next && DURATION.test(next) ? durationMs(next) : undefined;
      out.push({ artists: artistsOf(split[0]), title: split.slice(1).join(" - "), ...(length ? { durationMs: length } : {}) });
      continue;
    }
    // a VK row: title, artist, length
    const artist = lines[i + 1];
    const length = lines[i + 2];
    if (artist && length && DURATION.test(length) && !DURATION.test(artist)) {
      out.push({ title: cleaned, artists: artistsOf(clean(artist)), durationMs: durationMs(length) });
      i += 2;
    }
  }
  return out.filter((t) => t.title.length > 0 && t.title.length < 200);
}

/** pasted text that is a list of songs rather than a link or a code */
export function looksLikeTrackList(text: string): boolean {
  return !/^\s*(https?:\/\/|EVP[12]\.)/.test(text) && parseTrackList(text).length > 0;
}
