import { describe, expect, it } from "vitest";
import { outputLost } from "./session";

describe("output devices", () => {
  it("notices a named output that is gone", () => {
    expect(outputLost(["default", "headphones", "speakers"], ["default", "speakers"])).toBe(true);
    expect(outputLost(["default", "speakers"], ["default", "speakers", "headphones"])).toBe(false);
  });
  it("counts when the browser hides the ids", () => {
    expect(outputLost(["", ""], [""])).toBe(true);
    expect(outputLost([""], ["", ""])).toBe(false);
    expect(outputLost([""], [""])).toBe(false);
  });
});
