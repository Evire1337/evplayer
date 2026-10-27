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
