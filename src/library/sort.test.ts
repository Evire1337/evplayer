import { describe, expect, it } from "vitest";
import { sortRows } from "./sort";

const row = (id: string, title: string, artist: string, durationMs: number, addedAt: string) => ({
  track: { id, source: "soundcloud" as const, title, artists: [{ id: artist, name: artist }], durationMs },
  addedAt,
});
const rows = [row("a", "Яблоко", "Б", 200, "2026-01-02"), row("b", "apple", "А", 100, "2026-01-03"), row("c", "Банан", "В", 300, "2026-01-01")];

describe("sortRows", () => {
  it("sorts by each key", () => {
    expect(sortRows(rows, "title", "asc").map((r) => r.track.id)).toEqual(["b", "c", "a"]);
    expect(sortRows(rows, "duration", "desc").map((r) => r.track.id)).toEqual(["c", "a", "b"]);
    expect(sortRows(rows, "added", "desc").map((r) => r.track.id)).toEqual(["b", "a", "c"]);
    expect(sortRows(rows, "artist", "asc").map((r) => r.track.id)).toEqual(["b", "a", "c"]);
    expect(sortRows(rows, "plays", "desc", (id) => ({ a: 1, b: 5, c: 3 })[id] ?? 0).map((r) => r.track.id)).toEqual(["b", "c", "a"]);
    expect(sortRows(rows, "custom", "desc").map((r) => r.track.id)).toEqual(["c", "b", "a"]);
  });
});
