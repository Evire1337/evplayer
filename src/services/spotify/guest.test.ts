import { afterEach, describe, expect, it, vi } from "vitest";
import { extractSecret, mapItem, secretBytes, totp } from "./guest";

const SCRIPT = `let a=[{secret:'old',version:60},{secret:",7/*F(\\"x",version:61}]`;

afterEach(() => vi.unstubAllGlobals());

describe("spotify guest access", () => {
  it("picks the newest secret in the player script", () => {
    expect(extractSecret(SCRIPT)).toEqual({ secret: ',7/*F("x', version: 61 });
    expect(extractSecret("nothing here")).toBeUndefined();
  });

  it("makes six-digit codes that change every 30 seconds", async () => {
    const key = secretBytes("abc");
    const a = await totp(key, 0);
    expect(a).toMatch(/^\d{6}$/);
    expect(await totp(key, 29_000)).toBe(a);
  });

  it("turns a pathfinder row into a song to look up", () => {
    const row = {
      itemV2: {
        data: {
          __typename: "Track",
          name: " Song ",
          artists: { items: [{ profile: { name: "A" } }, { profile: { name: "B" } }] },
          trackDuration: { totalMilliseconds: 123000 },
        },
      },
    };
    expect(mapItem(row)).toEqual({ title: "Song", artists: ["A", "B"], durationMs: 123000 });
    expect(mapItem({ itemV2: { data: { __typename: "Episode", name: "x" } } })).toBeUndefined();
  });
});
