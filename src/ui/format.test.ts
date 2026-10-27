import { describe, expect, it } from "vitest";
import { formatCount, formatTime } from "./format";

describe("format", () => {
  it("formats durations", () => {
    expect(formatTime(0)).toBe("0:00");
    expect(formatTime(185_000)).toBe("3:05");
    expect(formatTime(3_725_000)).toBe("1:02:05");
  });
  it("formats counts in russian", () => {
    expect(formatCount(385_356)).toBe("385,4 тыс.");
    expect(formatCount(undefined)).toBeUndefined();
  });
});
