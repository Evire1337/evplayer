import type { Track } from "@/types";
import { searchSoundCloud, soundCloudStreams } from "@/services/soundcloud/api";
import { searchYouTubeMusic } from "@/services/youtube/music";
import { isOriginal, isSameRecording, matchScore, searchQueryFor, simplify, splitTitle } from "./match";
import { mediaUrl } from "@/files/localFiles";
import { keptUrl } from "@/offline/store";
import { resolveYouTubeAudio, videoIdOf } from "@/services/youtube/direct";

export interface PlayableStream {
  url: string;
  hls: boolean;
  /** the same song on the other service, playing because the original did not */
  via?: Track;
}

// remembered per session: searching once per song is enough
const matches = new Map<string, Track[]>();
/** how many uploads of the same song are tried (one can be protected, another not) */
const COPIES_TRIED = 3;
const youTubeMatches = new Map<string, Track | null>();

async function youTubeMatch(track: Track, signal?: AbortSignal): Promise<Track | null> {
  if (youTubeMatches.has(track.id)) return youTubeMatches.get(track.id) ?? null;
  // with the artist first; then the title alone (a SoundCloud uploader's name is often not the
  // artist's, and YouTube finds nothing for it)
  const queries = [...new Set([searchQueryFor(track), simplify(splitTitle(track).title)])].filter(Boolean);
  let found: Track | null = null;
  for (const query of queries) {
    const ranked = (await searchYouTubeMusic(query, signal))
      // YouTube Music songs are a catalogue: a SoundCloud uploader's name need not be the artist's
      .filter((t) => isSameRecording(track, t, { anyArtist: true }))
      // the artist's own release first, then the closest copy
      .sort((a, b) => Number(isOriginal(track, b)) - Number(isOriginal(track, a)) || matchScore(track, b) - matchScore(track, a));
    found = ranked[0] ?? null;
    if (found) break;
  }
  youTubeMatches.set(track.id, found);
  return found;
}

/** uploads of the same song on SoundCloud, most alike first */
async function soundCloudMatches(track: Track, signal?: AbortSignal): Promise<Track[]> {
  const known = matches.get(track.id);
  if (known) return known;
  const { tracks } = await searchSoundCloud(searchQueryFor(track), signal);
  const found = tracks
    .map((t) => ({ t, score: matchScore(track, t) }))
    // played in place of the song: the same recording, not a remix of about the same name
    .filter((x) => !x.t.preview && isSameRecording(track, x.t))
    .sort((a, b) => b.score - a.score)
    .slice(0, COPIES_TRIED)
    .map((x) => x.t);
  matches.set(track.id, found);
  return found;
}

/** a YouTube song with no copy on SoundCloud */
export class NoCopyError extends Error {
  constructor(cause?: unknown) {
    super(cause instanceof Error ? `Не получилось включить ни YouTube, ни копии с SoundCloud: ${cause.message}` : "Не получилось получить звук этой песни");
  }
}

/**
 * Every way to get a song's sound, best first; the player tries the next one when one does
 * not play (resolveStream below takes only the first):
 * - SoundCloud: each format of the upload, then the same song on YouTube (some uploads are
 *   only given out protected or as a 30-second preview, which the SoundCloud site can play
 *   and this player cannot)
 * - YouTube: straight from YouTube, then the song's copy on SoundCloud
 * A copy saved for offline listening goes before all of these.
 */
export async function* streamCandidates(track: Track, signal?: AbortSignal): AsyncGenerator<PlayableStream> {
  let lastError: unknown;
  // a copy saved for offline comes first: no waiting for a service, no connection needed
  const kept = keptUrl(track.id);
  if (kept) yield { url: kept, hls: false };
  switch (track.source) {
    case "local":
      yield { url: mediaUrl("track", track.id), hls: false };
      return;
    case "soundcloud": {
      // SoundCloud first, even for a song it listed as a preview: signed in with Go+, it gives
      // the whole song (only whole songs are taken: a 30-second cut never plays as the song).
      // Otherwise the full recording comes from YouTube Music, with the SoundCloud cover and names
      try {
        yield* soundCloudStreams(track.id, signal);
      } catch (e) {
        if (signal?.aborted) throw e;
        lastError = e;
      }
      const copy = await youTubeMatch(track, signal).catch((e) => {
        if (signal?.aborted) throw e;
        return null;
      });
      const videoId = copy && videoIdOf(copy.id);
      if (copy && videoId) {
        try {
          yield { url: (await resolveYouTubeAudio(videoId, signal)).url, hls: false, via: copy };
          return;
        } catch (e) {
          if (signal?.aborted) throw e;
          throw new Error(`копия с YouTube Music не включилась: ${e instanceof Error ? e.message : String(e)}`);
        }
      }
      // say both: what SoundCloud gives, and that there was no copy to play instead
      if (lastError) throw new Error(`${lastError instanceof Error ? lastError.message : String(lastError)}; на YouTube Music эта песня не нашлась`);
      if (track.preview) throw new Error("SoundCloud даёт только 30-секундный отрывок, а на YouTube Music эта песня не нашлась");
      return;
    }
    case "youtube": {
      const videoId = videoIdOf(track.id);
      if (videoId) {
        try {
          yield { url: (await resolveYouTubeAudio(videoId, signal)).url, hls: false };
        } catch (e) {
          if (signal?.aborted) throw e;
          console.warn("[youtube] direct playback failed, trying SoundCloud:", e);
        }
      }
      const copies = await soundCloudMatches(track, signal);
      if (!copies.length) throw new NoCopyError();
      // a copy that SoundCloud gives out only protected: the next one
      for (const copy of copies) {
        try {
          for await (const stream of soundCloudStreams(copy.id, signal)) yield { ...stream, via: copy };
        } catch (e) {
          if (signal?.aborted) throw e;
          lastError = e;
        }
      }
      throw new NoCopyError(lastError);
    }
  }
}

/** the first way to get a song's sound */
export async function resolveStream(track: Track, signal?: AbortSignal): Promise<PlayableStream> {
  for await (const stream of streamCandidates(track, signal)) return stream;
  throw new Error("Не нашлось, откуда взять звук этой песни");
}
