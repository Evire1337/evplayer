import { describe, expect, it } from "vitest";
import { bandResponseDb, defaultBands, freqToX, peakBoostDb, presetBands, totalResponseDb, xToFreq } from "./eqModel";

describe("eq model", () => {
  it("a peaking band reaches its gain at its own frequency", () => {
    const band = { id: "x", type: "peaking" as const, frequency: 1000, gain: 6, q: 1, enabled: true };
    expect(bandResponseDb(band, 1000)).toBeCloseTo(6, 1);
    expect(Math.abs(bandResponseDb(band, 50))).toBeLessThan(0.3);
  });

  it("shelves boost their side of the spectrum", () => {
    const low = { id: "l", type: "lowshelf" as const, frequency: 100, gain: 6, q: 0.71, enabled: true };
    const high = { ...low, id: "h", type: "highshelf" as const, frequency: 8000 };
    expect(bandResponseDb(low, 20)).toBeCloseTo(6, 0);
    expect(Math.abs(bandResponseDb(low, 10000))).toBeLessThan(0.2);
    expect(bandResponseDb(high, 18000)).toBeCloseTo(6, 0);
  });

  it("flat is flat and disabled bands do nothing", () => {
    expect(totalResponseDb(defaultBands(), 440)).toBe(0);
    const bands = presetBands("rich").map((b) => ({ ...b, enabled: false }));
    expect(peakBoostDb(bands)).toBe(0);
    expect(peakBoostDb(presetBands("rich"))).toBeGreaterThan(2);
  });

  it("maps the log frequency axis both ways", () => {
    expect(xToFreq(freqToX(1234, 600), 600)).toBeCloseTo(1234, 3);
  });
});
