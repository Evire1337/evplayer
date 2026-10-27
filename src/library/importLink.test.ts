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
    expect(linkKind("daft punk")).toBeNull();
    expect(linkKind("https://example.com/x")).toBeNull();
  });
});
