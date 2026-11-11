import { describe, expect, it } from "vitest";
import { artworkUrl, mapScTrack, pickTranscoding, qualityOf, rankTranscodings, trackAuthorization, transcodingsOf } from "./mappers";

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

  it("reads the per-track stream token", () => {
    expect(trackAuthorization({ track_authorization: "tok" })).toBe("tok");
    expect(trackAuthorization({})).toBeUndefined();
  });
  it("knows a SoundCloud Go+ song that is given out only as a preview", () => {
    const base = { kind: "track", id: 7, title: "Go+ song", full_duration: 200_000 };
    expect(mapScTrack({ ...base, duration: 30_000, policy: "SNIP" })?.preview).toBe(true);
    expect(mapScTrack({ ...base, duration: 30_000 })?.preview).toBe(true);
    expect(mapScTrack({ ...base, duration: 200_000, policy: "ALLOW" })?.preview).toBeUndefined();
    const formats = (duration: number) => ({ full_duration: 200_000, media: { transcodings: [{ url: "u", duration, format: { protocol: "progressive", mime_type: "audio/mpeg" } }] } });
    expect(transcodingsOf(formats(30_000))).toEqual([]);
    expect(transcodingsOf(formats(200_000))).toHaveLength(1);
  });

  it("tells the person what the sound is, and offers the better encoding first", () => {
    expect(qualityOf({ mimeType: "audio/mpeg", preset: "mp3_1_0" })).toBe("mp3 ~128 кбит/с");
    expect(qualityOf({ mimeType: 'audio/ogg; codecs="opus"', preset: "opus_0_0" })).toBe("opus ~64 кбит/с");
    expect(qualityOf({ mimeType: 'audio/mp4; codecs="mp4a.40.2"', preset: "aac_256k" })).toBe("aac 256 кбит/с");
    const list = [
      { url: "a", protocol: "progressive" as const, mimeType: "audio/mpeg", quality: "sq" },
      { url: "b", protocol: "hls" as const, mimeType: 'audio/mp4; codecs="mp4a.40.2"', quality: "hq" },
    ];
    expect(rankTranscodings(list).map((t) => t.url)).toEqual(["b", "a"]);
    // nothing says "hq": the order stays what it was
    expect(rankTranscodings(list.map(({ quality, ...t }) => t)).map((t) => t.url)).toEqual(["a", "b"]);
  });
});
