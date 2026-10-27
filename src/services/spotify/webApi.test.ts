import { beforeEach, describe, expect, it, vi } from "vitest";
import { useSettings } from "@/settings/store";
import { spotifyCollection } from "./webApi";

const track = (i: number) => ({ track: { name: `Song ${i}`, duration_ms: 1000, artists: [{ name: "A" }], type: "track" } });

describe("spotify web api", () => {
  beforeEach(() => {
    useSettings.getState().set({ spotifyAccount: { name: "me", accessToken: "t", refreshToken: "r", expiresAt: Date.now() + 60_000 } });
  });
  it("reads every page of a long playlist", async () => {
    const total = 250;
    const fetchMock = vi.fn(async (url: string) => {
      if (url.includes("?fields=name")) return new Response(JSON.stringify({ name: "Big", images: [{ url: "https://x/c.jpg" }] }));
      const offset = Number(new URL(url).searchParams.get("offset") ?? 0);
      const items = Array.from({ length: Math.min(100, total - offset) }, (_, i) => track(offset + i));
      const next = offset + 100 < total ? `https://api.spotify.com/v1/playlists/p/tracks?offset=${offset + 100}&limit=100` : null;
      return new Response(JSON.stringify({ items, next, total }));
    });
    vi.stubGlobal("fetch", fetchMock);
    const pages: number[] = [];
    const result = await spotifyCollection("playlist", "p", undefined, (n) => pages.push(n));
    expect(result.tracks.length).toBe(250);
    expect(result.tracks[249].title).toBe("Song 249");
    expect(pages).toEqual([100, 200, 250]);
    expect(result.artwork).toBe("https://x/c.jpg");
    vi.unstubAllGlobals();
  });
});
