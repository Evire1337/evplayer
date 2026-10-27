import { describe, expect, it } from "vitest";
import { isNewer } from "./phoneUpdates";

describe("isNewer", () => {
  it("compares versions part by part", () => {
    expect(isNewer("0.5.0", "0.4.2")).toBe(true);
    expect(isNewer("v0.4.10", "0.4.9")).toBe(true);
    expect(isNewer("0.4.2", "0.4.2")).toBe(false);
    expect(isNewer("0.4.1", "0.4.2")).toBe(false);
  });
});
