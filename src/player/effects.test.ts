import { describe, expect, it } from "vitest";
import { clampPitch, clampSpeed, DEFAULT_EFFECTS, effectivePitch, effectsForNextTrack } from "./effects";
import { ratioToSemitones, semitonesToRatio } from "@/audio/pitch";

describe("effects", () => {
  it("links pitch to speed like a record", () => {
    expect(effectivePitch({ ...DEFAULT_EFFECTS, speed: 2, linked: true })).toBeCloseTo(12, 5);
    expect(effectivePitch({ ...DEFAULT_EFFECTS, speed: 2, pitch: -3 })).toBe(-3);
  });
  it("clamps and rounds", () => {
    expect(clampSpeed(3)).toBe(2);
    expect(clampSpeed(1.234)).toBe(1.23);
    expect(clampPitch(-20)).toBe(-12);
  });
  it("resets for the next track unless kept", () => {
    const tuned = { ...DEFAULT_EFFECTS, speed: 1.25, reverb: true };
    expect(effectsForNextTrack(tuned).speed).toBe(1);
    expect(effectsForNextTrack({ ...tuned, keep: true }).speed).toBe(1.25);
  });
  it("converts semitones", () => {
    expect(semitonesToRatio(12)).toBeCloseTo(2, 6);
    expect(ratioToSemitones(0.5)).toBeCloseTo(-12, 6);
  });
});
