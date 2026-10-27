import type { Track } from "@/types";
import { resolveSoundCloudStream } from "@/services/soundcloud/api";

export interface PlayableStream {
  url: string;
  hls: boolean;
}

/** where the audio for a track comes from; new sources plug in here */
export async function resolveStream(track: Track, signal?: AbortSignal): Promise<PlayableStream> {
  switch (track.source) {
    case "soundcloud":
      return resolveSoundCloudStream(track.id, signal);
    default:
      throw new Error(`${track.source} playback is not ready yet`);
  }
}
