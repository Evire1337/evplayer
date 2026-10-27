import { describe, expect, it } from "vitest";
import { namedFromYandex, parseYandexLink } from "./music";

describe("yandex music links", () => {
  it("recognises playlists, albums, tracks and profiles", () => {
    expect(parseYandexLink("https://music.yandex.ru/users/evire/playlists/1003")).toEqual({ kind: "playlist", owner: "evire", id: "1003" });
    expect(parseYandexLink("https://music.yandex.ru/playlists/lk.0a1b2c3d-4e5f-6789-abcd-ef0123456789")).toEqual({ kind: "playlist-uuid", id: "lk.0a1b2c3d-4e5f-6789-abcd-ef0123456789" });
    expect(parseYandexLink("https://music.yandex.com/album/123/track/456?utm=x")).toEqual({ kind: "track", id: "456" });
    expect(parseYandexLink("https://music.yandex.ru/album/123")).toEqual({ kind: "album", id: "123" });
    expect(parseYandexLink("https://music.yandex.ru/users/evire")).toEqual({ kind: "user", owner: "evire" });
    expect(parseYandexLink("https://example.com/album/1")).toBeNull();
  });
  it("turns tracks into names", () => {
    expect(namedFromYandex({ title: "Song", version: "Remix", durationMs: 1000, artists: [{ name: "A" }, { name: "B" }] })).toEqual({
      title: "Song (Remix)",
      artists: ["A", "B"],
      durationMs: 1000,
    });
  });
});
