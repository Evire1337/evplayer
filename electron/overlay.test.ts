import { describe, expect, it, vi } from "vitest";
vi.mock("electron", () => ({ BrowserWindow: class {}, screen: {} }));
import { onSomeScreen, cornerPosition } from "./overlay";

describe("overlay position", () => {
  it("sits in the chosen corner of the work area", () => {
    const area = { x: 0, y: 0, width: 1920, height: 1040 };
    expect(cornerPosition("bottom-right", area)).toEqual({ x: 1920 - 330 - 24, y: 1040 - 52 - 24 });
    expect(cornerPosition("top-left", area)).toEqual({ x: 24, y: 24 });
    expect(cornerPosition("top-center", area)).toEqual({ x: (1920 - 330) / 2, y: 24 });
    expect(cornerPosition("bottom-center", area)).toEqual({ x: (1920 - 330) / 2, y: 1040 - 52 - 24 });
  });
});

describe("onSomeScreen", () => {
  const screens = [{ x: 0, y: 0, width: 1920, height: 1040 }, { x: 1920, y: 0, width: 1280, height: 1024 }];
  it("keeps a spot on either monitor", () => {
    expect(onSomeScreen({ x: 100, y: 100 }, screens)).toBe(true);
    expect(onSomeScreen({ x: 2500, y: 600 }, screens)).toBe(true);
  });
  it("drops a spot left behind when a monitor went away or shrank", () => {
    expect(onSomeScreen({ x: 2500, y: 600 }, screens.slice(0, 1))).toBe(false);
    expect(onSomeScreen({ x: 1500, y: 1000 }, [{ x: 0, y: 0, width: 1280, height: 720 }])).toBe(false);
  });
});
