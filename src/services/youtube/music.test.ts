import { describe, expect, it } from "vitest";
import fixture from "./fixture.test.json";
import { parseCount, parseDuration, parseSearch } from "./music";

describe("youtube music", () => {
  it("reads durations and counts", () => {
    expect(parseDuration("4:08")).toBe(248000);
    expect(parseDuration("1:02:03")).toBe(3723000);
    expect(parseDuration("abc")).toBe(0);
    expect(parseCount("58 млн просмотров")).toBe(58e6);
    expect(parseCount("385,4 тыс.")).toBe(385400);
    expect(parseCount("1.2M views")).toBe(1.2e6);
  });
  it("turns search results into tracks", () => {
    const tracks = parseSearch(fixture);
    expect(tracks.length).toBe(3);
    expect(tracks[0]).toMatchObject({ id: "yt:CCHdMIEGaaM", source: "youtube", durationMs: 248000, plays: 58e6 });
    expect(tracks[0].artists[0].name).toBe("convar HUN");
    expect(tracks[0].artwork).toMatch(/^https:/);
  });
});

import artistFixture from "./artist.test.json";
import { parseArtist } from "./music";

describe("youtube artist", () => {
  it("reads the header and the video cards", () => {
    const a = parseArtist(artistFixture);
    expect(a.name).toBe("Daft Punk");
    expect(a.monthlyListeners).toBe(67.8e6);
    expect(a.subscribers).toBe(7.2e6);
    expect(a.tracks.length).toBe(2);
    expect(a.tracks[0]).toMatchObject({ id: "yt:FGBhQbmPwH8", title: "One More Time", plays: 622e6 });
  });
});
