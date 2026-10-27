import { describe, expect, it } from "vitest";
import { decodePresetCode, encodePresetCode } from "./presetCode";
import { presetBands } from "./eqModel";

describe("preset codes", () => {
  it("round-trips a preset", () => {
    const bands = presetBands("rich");
    bands[2] = { ...bands[2], q: 2.5, enabled: false };
    const back = decodePresetCode(encodePresetCode({ name: "Мой звук", bands, preamp: -1.5 }));
    expect(back.name).toBe("Мой звук");
    expect(back.preamp).toBe(-1.5);
    expect(back.bands.map((b) => [b.type, b.frequency, b.gain, b.q, b.enabled])).toEqual(
      bands.map((b) => [b.type, b.frequency, b.gain, b.q, b.enabled]),
    );
  });
  it("rejects other text", () => {
    expect(() => decodePresetCode("hello")).toThrow();
  });
});

describe("advanced presets", () => {
  it("keep all ten bands through a code", async () => {
    const { advancedBands } = await import("./eqModel");
    const bands = advancedBands().map((b, i) => ({ ...b, gain: i - 4 }));
    const back = decodePresetCode(encodePresetCode({ name: "ten", preamp: -2, bands }));
    expect(back.bands).toHaveLength(10);
    expect(back.bands.map((b) => b.gain)).toEqual(bands.map((b) => b.gain));
  });
});
