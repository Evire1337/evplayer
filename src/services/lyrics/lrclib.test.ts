import { describe, expect, it } from "vitest";
import { pickRecord, syncedFits, type Record } from "./lrclib";

const song = { title: "Falseimager", artists: [{ id: "a", name: "Reku Mochizuki" }], durationMs: 200_000 };
const rec = (duration: number, extra: Partial<Record> = {}): Record => ({ trackName: "Falseimager", artistName: "Reku Mochizuki", duration, syncedLyrics: "[00:01.00] a", plainLyrics: "a", ...extra });

describe("LRCLIB record choice", () => {
  it("follows times only for a version of nearly the same length", () => {
    expect(syncedFits(rec(201), song.durationMs)).toBe(true);
    expect(syncedFits(rec(203), song.durationMs)).toBe(true);
    expect(syncedFits(rec(206), song.durationMs)).toBe(false);
    expect(syncedFits(rec(200, { syncedLyrics: null }), song.durationMs)).toBe(false);
    // the song's length is not known: nothing to compare, the times are taken
    expect(syncedFits(rec(180), 0)).toBe(true);
  });

  it("prefers the version with the closest length", () => {
    const far = rec(210, { plainLyrics: "far" });
    const near = rec(201, { plainLyrics: "near" });
    expect(pickRecord([far, near], song)?.record.plainLyrics).toBe("near");
    expect(pickRecord([far, near], song)?.synced).toBe(true);
  });

  it("falls back to the words alone when only a version of another length has times", () => {
    const pick = pickRecord([rec(212)], song);
    expect(pick?.synced).toBe(false);
    expect(pick?.record.plainLyrics).toBe("a");
  });

  it("drops a version that is too far off even for its words", () => {
    expect(pickRecord([rec(300)], song)).toBeNull();
  });
});
