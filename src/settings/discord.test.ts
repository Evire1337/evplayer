import { describe, expect, it } from "vitest";
import { DEFAULT_DISCORD_APP_ID, discordAppId, useSettings } from "./store";

const FOREIGN_APP_ID = "1561339119617974362";

describe("discord application id", () => {
  it("ships EvPlayer's own application id", () => {
    expect(DEFAULT_DISCORD_APP_ID).toMatch(/^\d{15,22}$/);
    expect(DEFAULT_DISCORD_APP_ID).not.toBe(FOREIGN_APP_ID);
  });

  it("prefers an id typed into the settings over the built-in one", () => {
    expect(discordAppId({ discordAppId: "  987654321098765432  " })).toBe("987654321098765432");
  });

  it("has Discord RPC switched on by default", () => {
    expect(useSettings.getState().discordEnabled).toBe(true);
  });
});
