import { describe, expect, it } from "vitest";
import { finalIndex } from "./useReorder";

describe("finalIndex", () => {
  it("places the item before or after the row it was dropped on", () => {
    expect(finalIndex(0, 3, true)).toBe(2);
    expect(finalIndex(0, 3, false)).toBe(3);
    expect(finalIndex(5, 1, true)).toBe(1);
    expect(finalIndex(5, 1, false)).toBe(2);
    expect(finalIndex(2, 2, false)).toBe(2);
  });
});
