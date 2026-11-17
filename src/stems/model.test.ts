import { describe, expect, it } from "vitest";
import { FLAT_STEMS, STEM_PRESETS, samePreset, stemGains, stemKey } from "./model";

describe("stem levels", () => {
  it("plays every track at its fader", () => {
    expect(stemGains({ vocals: 1, drums: 0.5, bass: 1.2, other: 1 }, [], null)).toEqual([1, 0.5, 1.2, 1]);
  });

  it("silences the muted ones", () => {
    expect(stemGains(FLAT_STEMS, ["vocals"], null)).toEqual([0, 1, 1, 1]);
  });

  it("plays only the solo one, and a muted solo is silence", () => {
    expect(stemGains(FLAT_STEMS, [], "bass")).toEqual([0, 0, 1, 0]);
    expect(stemGains(FLAT_STEMS, ["bass"], "bass")).toEqual([0, 0, 0, 0]);
  });

  it("keeps a fader in range", () => {
    expect(stemGains({ vocals: 9, drums: -1, bass: Number.NaN, other: 1 }, [], null)).toEqual([1.5, 0, 1, 1]);
  });

  it("knows which quick setting is on", () => {
    const noVocals = STEM_PRESETS[0];
    expect(samePreset(noVocals, FLAT_STEMS, ["vocals"], null)).toBe(true);
    expect(samePreset(noVocals, FLAT_STEMS, ["vocals", "bass"], null)).toBe(false);
    expect(samePreset(noVocals, FLAT_STEMS, ["vocals"], "drums")).toBe(false);
  });
});

describe("the key of a song", () => {
  it("is the same for the same song and differs for another", async () => {
    const a = await stemKey("sc:1", 200_000);
    expect(a).toMatch(/^[0-9a-f]{32}$/);
    expect(await stemKey("sc:1", 200_400)).toBe(a);
    expect(await stemKey("sc:2", 200_000)).not.toBe(a);
  });
});
