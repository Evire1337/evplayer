import { describe, expect, it } from "vitest";
import { activeLineIndex, parseLrc } from "./lrc";
import { normalizeTitle, readings } from "./lrclib";

describe("lrc", () => {
  it("parses timed lines, repeated stamps and skips tags", () => {
    const { synced, lines } = parseLrc("[ar:Someone]\n[00:01.50]Hello\n[00:05.00][00:20.00]Chorus\n[00:10]Bridge");
    expect(synced).toBe(true);
    expect(lines.map((l) => [l.timeMs, l.text])).toEqual([[1500, "Hello"], [5000, "Chorus"], [10000, "Bridge"], [20000, "Chorus"]]);
    expect(activeLineIndex(lines, 0)).toBe(-1);
    expect(activeLineIndex(lines, 6000)).toBe(1);
    expect(activeLineIndex(lines, 99999)).toBe(3);
  });
  it("keeps plain text as untimed lines", () => {
    expect(parseLrc("one\ntwo").synced).toBe(false);
  });
  it("reads titles the way uploads write them", () => {
    expect(normalizeTitle("Песня (Official Video) feat. X")).toBe("песня");
    expect(readings({ title: "Burial - Archangel", artists: [{ id: "x", name: "Some Channel" }] })[1]).toEqual({ title: "Archangel", artist: "Burial" });
  });
});
