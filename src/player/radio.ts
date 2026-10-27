import type { Track } from "@/types";
import { searchSoundCloud, soundCloudRelated } from "@/services/soundcloud/api";
import { searchQueryFor, simplify } from "./match";

// When the queue runs out (and autoplay is on), keep going with similar songs.

export async function similarTracks(seed: Track, exclude: Set<string>, signal?: AbortSignal): Promise<Track[]> {
  let found: Track[] = [];
  if (seed.source === "soundcloud") {
    found = await soundCloudRelated(seed.id, signal);
  } else {
    // find the song on SoundCloud, then ask for what is related to it
    const { tracks } = await searchSoundCloud(searchQueryFor(seed), signal);
    const first = tracks[0];
    found = first ? await soundCloudRelated(first.id, signal).catch(() => tracks) : [];
    if (!found.length && seed.artists[0]) found = (await searchSoundCloud(simplify(seed.artists[0].name), signal)).tracks;
  }
  return found.filter((t) => !exclude.has(t.id)).slice(0, 20);
}
