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

/** 0..1: how sure we are that `candidate` is the same recording as `wanted` */
export function matchScore(wanted: Track, candidate: Track): number {
  const w = splitTitle(wanted);
  const c = splitTitle(candidate);
  const titleScore = overlap(words(w.title), words(`${c.title} ${c.artist}`));
  const artistWords = words(w.artist);
  const artistScore = overlap(artistWords, words(`${candidate.artists.map((a) => a.name).join(" ")} ${candidate.title}`));
  let score = titleScore * 0.6 + artistScore * 0.3;
  if (wanted.durationMs && candidate.durationMs) {
    const diff = Math.abs(wanted.durationMs - candidate.durationMs);
    score += diff < 4000 ? 0.1 : diff < 10000 ? 0.05 : diff > 40000 ? -0.4 : 0;
  }
  // remixes, covers, sped-up versions are different songs unless that is what was asked for
  for (const tag of ["remix", "cover", "sped", "slowed", "nightcore", "live", "instrumental", "karaoke", "8d"]) {
    const inWanted = simplify(wanted.title).includes(tag);
    const inCandidate = simplify(candidate.title).includes(tag);
    if (inWanted !== inCandidate) score -= 0.3;
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
