import { describe, expect, it } from "vitest";
import { DRIFT_JUMP_S, DRIFT_OK_S, FLAT_STEMS, NUDGE, STEM_PRESETS, driftAction, samePreset, stemGains, stemKey } from "./model";

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

describe("keeping the tracks together", () => {
  it("leaves tracks that are close at their speed", () => {
    expect(driftAction(10, 10 + DRIFT_OK_S / 2, 1.25)).toEqual({ kind: "rate", rate: 1.25 });
  });

  it("nudges a track that is a little late or early", () => {
    expect(driftAction(10.1, 10, 1)).toEqual({ kind: "rate", rate: 1 + NUDGE });
    expect(driftAction(10, 10.1, 1)).toEqual({ kind: "rate", rate: 1 - NUDGE });
  });

  it("jumps a track that is far off", () => {
    expect(driftAction(20, 10, 1)).toEqual({ kind: "jump", to: 20 });
    expect(driftAction(10, 10 + DRIFT_JUMP_S + 0.1, 1)).toEqual({ kind: "jump", to: 10 });
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
