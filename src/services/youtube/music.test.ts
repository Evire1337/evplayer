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

import playlistsFixture from "./playlists.test.json";
import { parseCollections } from "./music";

describe("youtube collections", () => {
  it("reads playlist rows", () => {
    const list = parseCollections(playlistsFixture);
    expect(list.length).toBe(2);
    expect(list[0]).toMatchObject({ id: "ytb:VLPLRVVwMYsRVWw", kind: "playlist", title: "DAFT PUNK DISCOVERY" });
    expect(list[0].subtitle).toContain("Dowita 2610");
  });
});

import nextFixture from "./next.test.json";
import { parseUpNext } from "./music";

describe("youtube radio", () => {
  it("reads the up next list", () => {
    const list = parseUpNext(nextFixture);
    expect(list.length).toBe(3);
    expect(list[1]).toMatchObject({ id: "yt:pjSMPn8dEN0", title: "Snoh Aalegra - I Love You", durationMs: 207000, plays: 327000 });
    expect(list[1].artists[0].name).toBe("Snoh Aalegra");
  });
});

import { explicitFirst, isExplicit } from "./music";
import type { Track } from "@/types";

describe("uncensored versions", () => {
  const song = (id: string, title: string, explicit?: boolean): Track => ({ id, source: "youtube", title, artists: [{ id: "a", name: "Kendrick Lamar" }], durationMs: 1000, ...(explicit ? { explicit } : {}) });

  it("reads the explicit mark", () => {
    expect(isExplicit({ badges: [{ musicInlineBadgeRenderer: { icon: { iconType: "MUSIC_EXPLICIT_BADGE" } } }] })).toBe(true);
    expect(isExplicit({ badges: [{ musicInlineBadgeRenderer: { icon: { iconType: "OTHER" } } }] })).toBe(false);
    expect(isExplicit({})).toBe(false);
  });

  it("puts the uncensored version before its censored twin and leaves the rest", () => {
    const list = [song("yt:clean00000", "Money Trees"), song("yt:other00000", "Alright", true), song("yt:expl000000", "Money Trees", true)];
    expect(explicitFirst(list).map((t) => t.id)).toEqual(["yt:expl000000", "yt:clean00000", "yt:other00000"]);
    // no twin: the order stays
    expect(explicitFirst([song("yt:a0000000000", "X"), song("yt:b0000000000", "Y", true)]).map((t) => t.id)).toEqual(["yt:a0000000000", "yt:b0000000000"]);
  });
});
