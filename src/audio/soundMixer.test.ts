import { describe, expect, it } from "vitest";
import { DEEPEST_DB, FLAT, MAX_LEVEL, effectiveLevels, isFlat, levelToDb } from "./soundMixer";

describe("sound mixer levels", () => {
  it("does nothing at 100% everywhere", () => {
    expect(isFlat(FLAT)).toBe(true);
    expect(isFlat({ ...FLAT, voice: 0.99 })).toBe(false);
    expect(isFlat({ ...FLAT, solo: "bass" })).toBe(false);
  });

  it("turns a level into dB: 0 dB at 100%, the deepest cut at 0%, a lift above", () => {
    expect(levelToDb(1)).toBe(0);
    expect(levelToDb(0)).toBe(DEEPEST_DB);
    expect(levelToDb(0.5)).toBeCloseTo(-6.02, 1);
    expect(levelToDb(MAX_LEVEL)).toBeCloseTo(3.52, 1);
    // out of range and broken values stay sane
    expect(levelToDb(5)).toBeCloseTo(3.52, 1);
    expect(levelToDb(-1)).toBe(DEEPEST_DB);
    expect(levelToDb(Number.NaN)).toBe(0);
  });

  it("takes the muted ones to zero and keeps the solo", () => {
    const faders = { voice: 1.2, bass: 0.8, width: 1 };
    expect(effectiveLevels(faders, [], null)).toEqual({ voice: 1.2, bass: 0.8, width: 1, solo: null });
    expect(effectiveLevels(faders, ["voice"], null)).toEqual({ voice: 0, bass: 0.8, width: 1, solo: null });
    // a muted one that is also the solo: silence, whatever its fader says
    expect(effectiveLevels(faders, ["bass"], "bass")).toEqual({ voice: 1.2, bass: 0, width: 1, solo: "bass" });
  });
});
