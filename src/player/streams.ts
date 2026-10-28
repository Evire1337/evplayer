import type { Track } from "@/types";
import { resolveSoundCloudStream, searchSoundCloud } from "@/services/soundcloud/api";
import { bestMatch, searchQueryFor } from "./match";
import { mediaUrl } from "@/files/localFiles";
import { resolveYouTubeAudio, videoIdOf } from "@/services/youtube/direct";

export interface PlayableStream {
  url: string;
  hls: boolean;
  /** the SoundCloud upload a YouTube track was matched to */
  via?: Track;
}

// remembered per session: searching once per song is enough
const matches = new Map<string, Track | null>();

async function soundCloudMatch(track: Track, signal?: AbortSignal): Promise<Track | null> {
  if (matches.has(track.id)) return matches.get(track.id) ?? null;
  const { tracks } = await searchSoundCloud(searchQueryFor(track), signal);
  const found = bestMatch(track, tracks);
  matches.set(track.id, found);
  return found;
}

/** a YouTube song with no copy on SoundCloud */
export class NoCopyError extends Error {
  constructor() {
    super("Не получилось получить звук этой песни — пропускаю");
  }
}

/** where the audio for a track comes from; new sources plug in here */
export async function resolveStream(track: Track, signal?: AbortSignal): Promise<PlayableStream> {
  switch (track.source) {
    case "soundcloud":
      return resolveSoundCloudStream(track.id, signal);
    case "local":
      return { url: mediaUrl("track", track.id), hls: false };
    case "youtube": {
      // straight from YouTube first; if YouTube refuses, the song's copy on SoundCloud
      const videoId = videoIdOf(track.id);
      if (videoId) {
        try {
          const direct = await resolveYouTubeAudio(videoId, signal);
          return { url: direct.url, hls: false };
        } catch (e) {
          if (signal?.aborted) throw e;
          console.warn("[youtube] direct playback failed, trying SoundCloud:", e);
        }
      }
      const match = await soundCloudMatch(track, signal);
      if (!match) throw new NoCopyError();
      return { ...(await resolveSoundCloudStream(match.id, signal)), via: match };
    }
  }
}
