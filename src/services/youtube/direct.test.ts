import { describe, expect, it } from "vitest";
import { audioFormats, bestFormat, parseVisitorData, playabilityProblem, toStream, videoIdOf } from "./direct";

const reply = {
  playabilityStatus: { status: "OK" },
  streamingData: {
    adaptiveFormats: [
      { itag: 137, mimeType: 'video/mp4; codecs="avc1"', url: "https://x/v", bitrate: 9 },
      { itag: 140, mimeType: 'audio/mp4; codecs="mp4a.40.2"', url: "https://r.googlevideo.com/a?expire=1900000000", bitrate: 130000, contentLength: "3000000" },
      { itag: 251, mimeType: 'audio/webm; codecs="opus"', url: "https://r.googlevideo.com/b?expire=1900000000", bitrate: 140000, contentLength: "2500000" },
      { itag: 250, mimeType: 'audio/webm; codecs="opus"', signatureCipher: "s=abc", bitrate: 70000 },
    ],
  },
};

describe("youtube direct audio", () => {
  it("keeps only audio with a plain address and prefers opus 251", () => {
    const formats = audioFormats(reply);
    expect(formats.map((f) => f.itag)).toEqual([140, 251]);
    const best = bestFormat(formats)!;
    expect(best.itag).toBe(251);
    expect(toStream(best)).toMatchObject({ codec: "opus", mimeType: "audio/webm", contentLength: 2500000, expiresAt: 1900000000 * 1000 });
  });

  it("explains unplayable videos", () => {
    expect(playabilityProblem(reply)).toBeNull();
    expect(playabilityProblem({ playabilityStatus: { status: "ERROR", reason: "Video unavailable" } })).toBe("Video unavailable");
    expect(playabilityProblem({ playabilityStatus: { status: "LOGIN_REQUIRED" } })).toBe("YouTube: LOGIN_REQUIRED");
  });

  it("reads the visitor id from sw.js_data", () => {
    const id = "CgtBYmNkZWZnaGlqa2wiAbCdEfGh";
    expect(parseVisitorData(`)]}'\n[[[["x"]]]] ${id}`)).toBe(id);
    expect(parseVisitorData("nothing")).toBeNull();
  });

  it("accepts yt: ids and bare video ids only", () => {
    expect(videoIdOf("yt:dQw4w9WgXcQ")).toBe("dQw4w9WgXcQ");
    expect(videoIdOf("sc:123")).toBeNull();
    expect(videoIdOf("name:song")).toBeNull();
  });
});
