import type { Track } from "@/types";

// Finding the same song on another service: YouTube tracks first look for the SoundCloud
// upload, so they play through our own audio path (equalizer, speed, reverb all work).

const NOISE = /\((?:official|lyrics?|audio|video|music video|hd|hq|4k|visuali[sz]er|клип|текст|премьера)[^)]*\)|\[(?:[^\]]*)\]|official (?:music )?video|lyric video|official audio/gi;

export function simplify(value: string): string {
  return value
    .toLowerCase()
    .replace(NOISE, " ")
    .replace(/\b(?:feat|ft|prod)\.?\s.*$/i, " ")
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();
}

function words(value: string): Set<string> {
  return new Set(simplify(value).split(" ").filter((w) => w.length > 1 || /\d/.test(w)));
}

function overlap(a: Set<string>, b: Set<string>): number {
  if (!a.size || !b.size) return 0;
  let shared = 0;
  for (const w of a) if (b.has(w)) shared++;
  return shared / Math.min(a.size, b.size);
}

/** "Artist - Title" uploads put the artist into the title: split it back out */
export function splitTitle(track: Pick<Track, "title" | "artists">): { title: string; artist: string } {
  const m = /^(.+?)\s+[-–—]\s+(.+)$/.exec(track.title);
  if (m) return { artist: m[1], title: m[2] };
  return { artist: track.artists[0]?.name ?? "", title: track.title };
}

/** a whole word (or words) in any alphabet: \b only knows Latin letters */
const word = (pattern: string) => new RegExp(`(?<![\\p{L}\\p{N}])(?:${pattern})(?![\\p{L}\\p{N}])`, "u");

/** markers of another version of a song: a remix, a cover, sped up, an edit… */
const VERSION_TAGS: Array<[string, RegExp]> = [
  ["remix", word("remix|rmx|ремикс")],
  ["cover", word("cover|кавер")],
  ["sped", word("sped|speed ?up|ускор\\p{L}*")],
  ["slowed", word("slowed|замедл\\p{L}*")],
  ["reverb", word("reverb")],
  ["nightcore", word("nightcore")],
  ["live", word("live|концерт\\p{L}*")],
  ["instrumental", word("instrumental|минус|минусовка")],
  ["karaoke", word("karaoke|караоке")],
  ["8d", word("8d")],
  // "radio edit" / "clean edit" are the same song
  ["edit", word("(?<!radio |clean |explicit )edit")],
  ["bootleg", word("bootleg")],
  ["flip", word("flip")],
  ["mashup", word("mash ?up|мэшап")],
  ["phonk", word("phonk|фонк")],
  ["vip", word("vip")],
  ["rework", word("rework|remake")],
  ["bass", word("bass ?boost(?:ed)?")],
  ["acoustic", word("acoustic|акустик\\p{L}*")],
  // "Club Mix", "Extended Mix", but not "Original Mix"
  ["mix", word("(?<!original )mix")],
];

export function versionTags(title: string): Set<string> {
  const t = title.toLowerCase().replace(/ё/g, "е");
  return new Set(VERSION_TAGS.filter(([, re]) => re.test(t)).map(([tag]) => tag));
}

/**
 * Good enough to play in place of the song: a close match of about the same length.
 * (bestMatch's threshold is for finding the song at all, e.g. when importing a list.)
 */
export function isSameRecording(wanted: Track, candidate: Track): boolean {
  const diff = wanted.durationMs && candidate.durationMs ? Math.abs(wanted.durationMs - candidate.durationMs) : Infinity;
  if (diff !== Infinity && diff > 15000) return false;
  if (matchScore(wanted, candidate) >= 0.8) return true;
  // the very same title, as long to a few seconds, and not another version: the same song even
  // when the artist is written differently (a SoundCloud uploader's name is often not the artist's)
  return diff < 3000 && titleScore(wanted, candidate) === 1 && sameVersion(wanted, candidate);
}

function sameVersion(wanted: Track, candidate: Track): boolean {
  const a = versionTags(wanted.title);
  const b = versionTags(candidate.title);
  return a.size === b.size && [...a].every((tag) => b.has(tag));
}

/** 0..1: all of the wanted title is in the candidate's, and the candidate's has nothing much besides */
function titleScore(wanted: Track, candidate: Track): number {
  const w = splitTitle(wanted);
  const c = splitTitle(candidate);
  // both ways: "Love" is not "Love Me Harder" by the same artist, though every word is there
  const wantedTitle = words(w.title);
  const found = overlap(wantedTitle, words(`${c.title} ${c.artist}`));
  const credited = words(`${c.artist} ${candidate.artists.map((a) => a.name).join(" ")}`);
  const named = new Set([...words(c.title)].filter((x) => !credited.has(x)));
  const extra = named.size ? [...named].filter((x) => wantedTitle.has(x) || words(w.artist).has(x)).length / named.size : 1;
  return found && extra ? (2 * found * extra) / (found + extra) : 0;
}

/** like overlap, but a name inside a longer one counts ("mikeey" in "mikeeysmind") */
function nameOverlap(a: Set<string>, b: Set<string>): number {
  if (!a.size || !b.size) return 0;
  let shared = 0;
  for (const w of a) if ([...b].some((x) => x === w || (Math.min(x.length, w.length) >= 4 && (x.startsWith(w) || w.startsWith(x))))) shared++;
  return shared / Math.min(a.size, b.size);
}

/** 0..1: how sure we are that `candidate` is the same recording as `wanted` */
export function matchScore(wanted: Track, candidate: Track): number {
  const w = splitTitle(wanted);
  const artistWords = words(w.artist);
  const artistScore = nameOverlap(artistWords, words(`${candidate.artists.map((a) => a.name).join(" ")} ${candidate.title}`));
  let score = titleScore(wanted, candidate) * 0.6 + artistScore * 0.3;
  if (wanted.durationMs && candidate.durationMs) {
    // a remix or an edit is usually a different length: that counts for a lot
    const diff = Math.abs(wanted.durationMs - candidate.durationMs);
    score += diff < 4000 ? 0.1 : diff < 8000 ? 0.05 : diff < 15000 ? 0 : diff < 30000 ? -0.2 : -0.5;
  }
  // remixes, covers, sped-up versions are different songs unless that is what was asked for;
  // read from the raw title: tags are often in brackets, which simplify() drops
  const wantedTags = versionTags(wanted.title);
  const candidateTags = versionTags(candidate.title);
  for (const tag of new Set([...wantedTags, ...candidateTags])) {
    if (wantedTags.has(tag) !== candidateTags.has(tag)) score -= 0.35;
  }
  return score;
}

export function bestMatch(wanted: Track, candidates: Track[], threshold = 0.72): Track | null {
  let best: Track | null = null;
  let bestScore = threshold;
  for (const candidate of candidates) {
    const score = matchScore(wanted, candidate);
    if (score >= bestScore) {
      best = candidate;
      bestScore = score;
    }
  }
  return best;
}

export function searchQueryFor(track: Pick<Track, "title" | "artists">): string {
  const { artist, title } = splitTitle(track);
  return `${simplify(artist)} ${simplify(title)}`.trim();
}
