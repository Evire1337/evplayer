import { describe, expect, it } from "vitest";
import { linkKind } from "./importLink";

describe("linkKind", () => {
  it("recognises service links", () => {
    expect(linkKind("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=1")).toBe("spotify");
    expect(linkKind("https://music.youtube.com/playlist?list=PL123")).toBe("youtube");
    expect(linkKind("https://youtu.be/abc")).toBe("youtube");
    expect(linkKind("https://soundcloud.com/a/sets/b")).toBe("soundcloud");
    expect(linkKind("https://www.deezer.com/en/playlist/908622995")).toBe("deezer");
    expect(linkKind("EVP2.abc")).toBe("code");
    expect(linkKind("https://music.yandex.ru/users/x/playlists/3")).toBe("yandex");
    expect(linkKind("daft punk")).toBeNull();
    expect(linkKind("https://example.com/x")).toBeNull();
  });
});

import { parseCounts } from "./importProgress";
import { decodePlaylistCode, encodePlaylistCode } from "./playlistCode";

describe("import progress", () => {
  it("reads the counts out of a step", () => {
    expect(parseCounts("Ищу песни: 12 из 80 (найдено 10)")).toEqual({ done: 12, total: 80, found: 10 });
    expect(parseCounts("Читаю ссылку…")).toEqual({ done: null, total: null, found: null });
  });

  it("a likes code says it is the likes", async () => {
    const t = { id: "sc:1", source: "soundcloud" as const, title: "A", artists: [], durationMs: 1 };
    expect((await decodePlaylistCode(await encodePlaylistCode({ title: "Понравившиеся", likes: true, tracks: [t] }))).likes).toBe(true);
    expect((await decodePlaylistCode(await encodePlaylistCode({ title: "P", tracks: [t] }))).likes).toBeUndefined();
  });
});
