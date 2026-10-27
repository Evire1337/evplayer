import { describe, expect, it } from "vitest";
import { artworkUrl, mapScTrack, pickTranscoding, transcodingsOf } from "./mappers";

describe("soundcloud mappers", () => {
  it("maps a track and asks for big artwork", () => {
    const track = mapScTrack({
      kind: "track",
      id: 42,
      title: "Song",
      duration: 1000,
      full_duration: 180000,
      playback_count: 1234,
      artwork_url: "https://i1.sndcdn.com/artworks-abc-large.jpg",
      user: { id: 7, username: "uploader" },
      publisher_metadata: { artist: "Real Artist" },
    });
    expect(track).toMatchObject({ id: "sc:42", durationMs: 180000, plays: 1234, artists: [{ id: "scu:7", name: "Real Artist" }] });
    expect(track?.artwork).toBe("https://i1.sndcdn.com/artworks-abc-t500x500.jpg");
    expect(artworkUrl(undefined)).toBeUndefined();
  });

  it("prefers progressive mp3 and skips encrypted streams", () => {
    const list = transcodingsOf({
      media: {
        transcodings: [
          { url: "a", format: { protocol: "hls", mime_type: "audio/ogg; codecs=\"opus\"" } },
          { url: "b", format: { protocol: "ctr-encrypted-hls", mime_type: "audio/mpeg" } },
          { url: "c", format: { protocol: "progressive", mime_type: "audio/mpeg" } },
          { url: "d", format: { protocol: "hls", mime_type: "audio/mpeg" } },
        ],
      },
    });
    expect(list.map((t) => t.url)).toEqual(["a", "c", "d"]);
    expect(pickTranscoding(list)?.url).toBe("c");
    expect(pickTranscoding(list.filter((t) => t.url !== "c"))?.url).toBe("d");
  });
});
