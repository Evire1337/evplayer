import { describe, expect, it } from "vitest";
import { LoudnessLeveler, MAX_BOOST_DB, TARGET_DB } from "./loudness";

const powerOf = (db: number) => Math.pow(10, db / 10);

describe("loudness leveller", () => {
  it("turns a loud song down and a quiet one up toward the target", () => {
    const loud = new LoudnessLeveler();
    const quiet = new LoudnessLeveler();
    for (let t = 0; t < 30_000; t += 150) {
      loud.update(powerOf(-10), 150);
      quiet.update(powerOf(-24), 150);
    }
    expect(loud.gainDb()).toBeCloseTo(TARGET_DB + 10, 0);
    expect(quiet.gainDb()).toBeCloseTo(TARGET_DB + 24, 0);
  });

  it("ignores silence and caps the boost", () => {
    const leveler = new LoudnessLeveler();
    leveler.update(powerOf(-80), 150);
    expect(leveler.gainDb()).toBe(0);
    for (let t = 0; t < 20_000; t += 150) leveler.update(powerOf(-45), 150);
    expect(leveler.gainDb()).toBe(MAX_BOOST_DB);
  });

  it("starts from a remembered level", () => {
    expect(new LoudnessLeveler(-12).gainDb()).toBeCloseTo(-6, 5);
  });
});
