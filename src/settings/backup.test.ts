import { beforeEach, describe, expect, it } from "vitest";
import { makeBackup, readBackup, restoreBackup } from "./backup";

describe("backup", () => {
  beforeEach(() => localStorage.clear());
  it("round-trips EvPlayer keys only", () => {
    localStorage.setItem("evplayer.library.v1", "{\"a\":1}");
    localStorage.setItem("evplayer.loudness.v1", "x");
    localStorage.setItem("other", "y");
    const backup = readBackup(JSON.stringify(makeBackup()));
    expect(Object.keys(backup.data)).toEqual(["evplayer.library.v1"]);
    localStorage.setItem("evplayer.library.v1", "changed");
    localStorage.setItem("evplayer.extra", "z");
    expect(restoreBackup(backup)).toBe(1);
    expect(localStorage.getItem("evplayer.library.v1")).toBe("{\"a\":1}");
    expect(localStorage.getItem("evplayer.extra")).toBeNull();
    expect(localStorage.getItem("other")).toBe("y");
  });
  it("rejects other files", () => {
    expect(() => readBackup("{}")).toThrow();
  });
});
