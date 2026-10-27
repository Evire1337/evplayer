import { afterEach, describe, expect, it, vi } from "vitest";
import { resolveSoundCloudStream } from "./api";

describe("resolveSoundCloudStream", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("sends the track token and falls back to the next format", async () => {
    vi.stubGlobal("evplayer", { soundcloudClientId: async () => "cid" });
    (window as unknown as { evplayer: unknown }).evplayer = { soundcloudClientId: async () => "cid" };
    const calls: string[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: URL) => {
        const url = new URL(String(input));
        calls.push(url.pathname + url.search);
        if (url.pathname === "/tracks/5") {
          return new Response(
            JSON.stringify({
              track_authorization: "tok",
              media: {
                transcodings: [
                  { url: "https://api-v2.soundcloud.com/media/p", format: { protocol: "progressive", mime_type: "audio/mpeg" } },
                  { url: "https://api-v2.soundcloud.com/media/h", format: { protocol: "hls", mime_type: "audio/mpeg" } },
                ],
              },
            }),
          );
        }
        if (url.pathname === "/media/p") return new Response("", { status: 404 });
        return new Response(JSON.stringify({ url: "https://cdn/stream.m3u8" }));
      }),
    );
    const stream = await resolveSoundCloudStream("sc:5");
    expect(stream).toEqual({ url: "https://cdn/stream.m3u8", hls: true });
    expect(calls.some((c) => c.startsWith("/media/p") && c.includes("track_authorization=tok"))).toBe(true);
  });
});
