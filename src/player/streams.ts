import type { Track } from "@/types";
import { resolveSoundCloudStream, searchSoundCloud } from "@/services/soundcloud/api";
import { bestMatch, searchQueryFor } from "./match";
import { mediaUrl } from "@/files/localFiles";

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
    super("Этой песни нет на SoundCloud — пропускаю");
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
      // YouTube songs play from their copy on SoundCloud; there is no other way to get the sound
      const match = await soundCloudMatch(track, signal);
      if (!match) throw new NoCopyError();
      return { ...(await resolveSoundCloudStream(match.id, signal)), via: match };
    }
  }
}
