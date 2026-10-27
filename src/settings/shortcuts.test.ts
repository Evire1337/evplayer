import { describe, expect, it } from "vitest";
import { acceleratorFromEvent, appAction, conflictFor, globalMap } from "./shortcuts";

const ev = (code: string, mods: Partial<{ ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }> = {}) => ({ code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods });

describe("shortcuts", () => {
  it("reads keys independent of layout", () => {
    expect(acceleratorFromEvent(ev("KeyL"))).toBe("L");
    expect(acceleratorFromEvent(ev("ArrowRight", { ctrlKey: true }))).toBe("Ctrl+Right");
    expect(acceleratorFromEvent(ev("Comma", { ctrlKey: true }))).toBe("Ctrl+,");
    expect(acceleratorFromEvent(ev("ShiftLeft", { shiftKey: true }))).toBeNull();
  });
  it("finds actions, conflicts and global keys", () => {
    expect(appAction({}, "Space")).toBe("playPause");
    expect(appAction({ "app.playPause": "P" }, "Space")).toBeNull();
    expect(appAction({ "app.playPause": "P" }, "P")).toBe("playPause");
    expect(conflictFor({}, "app.like", "M")?.id).toBe("app.mute");
    expect(conflictFor({}, "global.like", "M")).toBeUndefined();
    expect(globalMap({ "global.next": null })["Ctrl+Alt+Right"]).toBeUndefined();
    expect(globalMap({})["Ctrl+Alt+Space"]).toBe("playPause");
  });
});
