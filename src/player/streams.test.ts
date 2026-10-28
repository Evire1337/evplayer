import { afterEach, describe, expect, it, vi } from "vitest";
import type { Track } from "@/types";

vi.mock("@/services/soundcloud/api", () => ({
  searchSoundCloud: vi.fn(async () => ({ tracks: [], artists: [] })),
  soundCloudStreams: vi.fn(async function* () {
    yield { url: "https://sc/a.mp3", hls: false };
    yield { url: "https://sc/b.m3u8", hls: true };
  }),
}));
vi.mock("@/services/youtube/music", () => ({
  searchYouTubeMusic: vi.fn(async () => [
    { id: "yt:abcdefghijk", source: "youtube", title: "Song", artists: [{ id: "a", name: "Artist" }], durationMs: 200_000 },
  ]),
}));
vi.mock("@/services/youtube/direct", () => ({
  resolveYouTubeAudio: vi.fn(async () => ({ url: "https://yt/audio" })),
  videoIdOf: (id: string) => (id.startsWith("yt:") ? id.slice(3) : null),
}));

import { streamCandidates } from "./streams";
import { soundCloudStreams } from "@/services/soundcloud/api";

const sc: Track = { id: "sc:1", source: "soundcloud", title: "Song", artists: [{ id: "a", name: "Artist" }], durationMs: 200_000 };

async function all(track: Track) {
  const out = [];
  for await (const s of streamCandidates(track)) out.push(s);
  return out;
}

describe("streamCandidates", () => {
  afterEach(() => vi.clearAllMocks());

  it("a SoundCloud song: every format, then the same song on YouTube", async () => {
    const list = await all(sc);
    expect(list.map((s) => s.url)).toEqual(["https://sc/a.mp3", "https://sc/b.m3u8", "https://yt/audio"]);
    expect(list[2].via?.source).toBe("youtube");
  });

  it("a SoundCloud song with nothing playable still gets its YouTube copy", async () => {
    vi.mocked(soundCloudStreams).mockImplementationOnce(async function* () {
      throw new Error("protected");
    });
    const list = await all({ ...sc, id: "sc:2" });
    expect(list.map((s) => s.url)).toEqual(["https://yt/audio"]);
  });
});
