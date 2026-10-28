import { afterEach, describe, expect, it, vi } from "vitest";
import { resolveSoundCloudStream, soundCloudStreams } from "./api";

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

  it("plays Opus-in-HLS through the media server, joined into one file", async () => {
    (window as unknown as { evplayer: unknown }).evplayer = { soundcloudClientId: async () => "cid" };
    window.__EVPLAYER_MEDIA__ = "http://127.0.0.1:1/s";
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: URL) => {
        const url = new URL(String(input));
        if (url.pathname === "/tracks/7") {
          return new Response(
            JSON.stringify({
              media: {
                transcodings: [
                  { url: "https://api-v2.soundcloud.com/media/enc", format: { protocol: "ctr-encrypted-hls", mime_type: "audio/mp4" } },
                  { url: "https://api-v2.soundcloud.com/media/opus", format: { protocol: "hls", mime_type: "audio/ogg; codecs=\"opus\"" } },
                ],
              },
            }),
          );
        }
        return new Response(JSON.stringify({ url: "https://cf-hls-opus-media.sndcdn.com/playlist/x.m3u8" }));
      }),
    );
    const out = [];
    for await (const s of soundCloudStreams("sc:7")) out.push(s);
    expect(out).toEqual([{ url: `http://127.0.0.1:1/s/joined/${encodeURIComponent("https://cf-hls-opus-media.sndcdn.com/playlist/x.m3u8")}`, hls: false }]);
    delete window.__EVPLAYER_MEDIA__;
  });
});
