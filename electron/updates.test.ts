import { describe, expect, it, vi } from "vitest";
vi.mock("electron", () => ({ app: {}, ipcMain: { handle: () => {} } }));
vi.mock("electron-updater", () => ({ default: { autoUpdater: {} }, autoUpdater: {} }));
import { shortUpdateError } from "./updates";

describe("update errors", () => {
  it("are short and readable", () => {
    expect(shortUpdateError("Cannot find latest.yml in the latest release artifacts (https://x/latest.yml): HttpError: 404 ...")).toContain("latest.yml");
    expect(shortUpdateError("net::ERR_INTERNET_DISCONNECTED")).toBe("нет связи с GitHub");
    expect(shortUpdateError("x".repeat(300)).length).toBeLessThanOrEqual(140);
  });
});
