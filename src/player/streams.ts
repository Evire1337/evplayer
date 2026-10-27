import type { Track } from "@/types";
import { resolveSoundCloudStream, searchSoundCloud } from "@/services/soundcloud/api";
import { useSettings } from "@/settings/store";
import { bestMatch, searchQueryFor } from "./match";

export type PlayableStream =
  | { kind: "audio"; url: string; hls: boolean; /** the SoundCloud upload a YouTube track was matched to */ via?: Track }
  | { kind: "embed"; videoId: string };

// remembered per session: searching once per song is enough
const matches = new Map<string, Track | null>();

async function soundCloudMatch(track: Track, signal?: AbortSignal): Promise<Track | null> {
  if (matches.has(track.id)) return matches.get(track.id) ?? null;
  const { tracks } = await searchSoundCloud(searchQueryFor(track), signal);
  const found = bestMatch(track, tracks);
  matches.set(track.id, found);
  return found;
}

/** where the audio for a track comes from; new sources plug in here */
export async function resolveStream(track: Track, signal?: AbortSignal): Promise<PlayableStream> {
  switch (track.source) {
    case "soundcloud":
      return { kind: "audio", ...(await resolveSoundCloudStream(track.id, signal)) };
    case "local":
      return { kind: "audio", url: `evplayer-media://track/${encodeURIComponent(track.id)}`, hls: false };
    case "youtube": {
      const videoId = track.id.slice(3);
      if (useSettings.getState().youtubeMatchSoundCloud) {
        try {
          const match = await soundCloudMatch(track, signal);
          if (match) return { kind: "audio", ...(await resolveSoundCloudStream(match.id, signal)), via: match };
        } catch (error) {
          if (signal?.aborted) throw error;
          // SoundCloud is down or has nothing: fall back to the YouTube player
        }
      }
      return { kind: "embed", videoId };
    }
  }
}
