import { describe, expect, it } from "vitest";
import { bestMatch, isSameRecording, matchScore, searchQueryFor, simplify, versionTags } from "./match";
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
  it("does not take a longer title by the same artist for the song", () => {
    const yt = { ...tr("Love", "Kendrick Lamar", 213000), source: "youtube" as const };
    expect(isSameRecording(yt, tr("Love Me Harder", "Kendrick Lamar", 214000))).toBe(false);
    expect(isSameRecording(yt, tr("LOVE.", "Kendrick Lamar", 213500))).toBe(true);
    expect(isSameRecording(yt, tr("Kendrick Lamar - Love (Official Audio)", "kendricklamar", 213000))).toBe(true);
  });
  it("builds a search query", () => {
    expect(searchQueryFor(tr("Daft Punk - Get Lucky (Official Video)", "convar"))).toBe("daft punk get lucky");
  });
  it("sees version tags in brackets too", () => {
    expect([...versionTags("Song [Phonk Remix]")].sort()).toEqual(["phonk", "remix"]);
    expect(versionTags("Song (Radio Edit)").has("edit")).toBe(false);
    expect(versionTags("Song (Original Mix)").has("mix")).toBe(false);
    expect(versionTags("Песня (ускоренная версия)").has("sped")).toBe(true);
  });
  it("does not play a remix in place of the original", () => {
    const yt = { ...tr("He's back", "OUENZA", 184000), source: "youtube" as const };
    expect(isSameRecording(yt, tr("OUENZA - He's back [Phonk Remix]", "someone", 186000))).toBe(false);
    expect(isSameRecording(yt, tr("He's back", "OUENZA", 212000))).toBe(false);
    expect(isSameRecording(yt, tr("He's Back", "OUENZA", 185000))).toBe(true);
  });
});
