import { describe, expect, it, vi } from "vitest";
vi.mock("electron", () => ({ BrowserWindow: class {}, screen: {} }));
import { cornerPosition } from "./overlay";

describe("overlay position", () => {
  it("sits in the chosen corner of the work area", () => {
    const area = { x: 0, y: 0, width: 1920, height: 1040 };
    expect(cornerPosition("bottom-right", area)).toEqual({ x: 1920 - 330 - 24, y: 1040 - 52 - 24 });
    expect(cornerPosition("top-left", area)).toEqual({ x: 24, y: 24 });
  });
});
