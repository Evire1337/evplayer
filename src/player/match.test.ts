import { describe, expect, it } from "vitest";
import { bestMatch, matchScore, searchQueryFor, simplify } from "./match";
import type { Track } from "@/types";

const tr = (title: string, artist: string, durationMs = 200000, id = title): Track => ({
  id, source: "soundcloud", title, artists: [{ id: artist, name: artist }], durationMs,
});

describe("match", () => {
  it("cleans titles", () => {
    expect(simplify("Get Lucky (Official Video) feat. Pharrell")).toBe("get lucky");
  });
  it("prefers the same song with a similar length", () => {
    const yt = { ...tr("Daft Punk - Get Lucky (Official Video)", "Daft Punk", 248000), source: "youtube" as const };
    const good = tr("Get Lucky", "Daft Punk", 247000, "good");
    const remix = tr("Get Lucky (Remix)", "Daft Punk", 300000, "remix");
    const other = tr("Lose Yourself to Dance", "Daft Punk", 250000, "other");
    expect(matchScore(yt, good)).toBeGreaterThan(matchScore(yt, remix));
    expect(bestMatch(yt, [other, remix, good])?.id).toBe("good");
    expect(bestMatch(yt, [other])).toBeNull();
  });
  it("builds a search query", () => {
    expect(searchQueryFor(tr("Daft Punk - Get Lucky (Official Video)", "convar"))).toBe("daft punk get lucky");
  });
});
