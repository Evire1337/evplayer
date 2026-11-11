import { describe, expect, it } from "vitest";
import { bestMatch, isOriginal, isSameRecording, matchScore, searchQueryFor, simplify, versionTags } from "./match";
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
  it("takes a copy under another artist name when the title and length are the same", () => {
    const sc = tr("VVV (HE'S BACK)", "mikeeysmind", 80000);
    expect(isSameRecording(sc, { ...tr("VVV (HE'S BACK)", "Mikeey", 81000), source: "youtube" })).toBe(true);
    expect(isSameRecording(sc, { ...tr("VVV (HE'S BACK)", "Someone Else", 81000), source: "youtube" })).toBe(true);
    expect(isSameRecording(sc, { ...tr("VVV (HE'S BACK)", "Someone Else", 95000), source: "youtube" })).toBe(false);
    expect(isSameRecording(sc, { ...tr("VVV (HE'S BACK) (Sped Up)", "Mikeey", 81000), source: "youtube" })).toBe(false);
    // what was found on YouTube for it: a re-upload of the same part, and the artist's original (longer)
    expect(isSameRecording(sc, { ...tr("VVV HE'S BACK (ALL CREDITS TO mikeeysmind) not my song", "CYBERMASK", 81000), source: "youtube" })).toBe(true);
    const original = { ...tr("VVV", "mikeeysmind", 228000), source: "youtube" as const };
    expect(isSameRecording(sc, original)).toBe(true);
    expect(isOriginal(sc, original)).toBe(true);
    // the original is the artist's: someone else's "VVV", or another of the artist's songs, is not
    expect(isOriginal(sc, { ...original, artists: [{ id: "x", name: "WizNation" }] })).toBe(false);
    expect(isOriginal(tr("Love Story", "Artist"), tr("Love", "Artist", 100000))).toBe(false);
    expect(isOriginal(sc, { ...original, title: "VVV (Remix)" })).toBe(false);
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
