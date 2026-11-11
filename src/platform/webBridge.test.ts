import { describe, expect, it } from "vitest";
import { MEDIA_PATH, PROXY_PATH, targetOf } from "../../tools/webBridge";

// The dev server's stand-in for the desktop side (tools/webBridge.ts) serves only the hosts the
// app itself may reach.

const enc = (url: string) => encodeURIComponent(url);

describe("web bridge addresses", () => {
  it("serves a service address, asked for through the proxy or the media route", () => {
    const url = "https://api-v2.soundcloud.com/search/tracks?q=a b&client_id=1";
    expect(targetOf(PROXY_PATH + enc(url))).toBe(url);
    expect(targetOf(MEDIA_PATH + enc("https://cf-media.sndcdn.com/x.mp3"))).toBe("https://cf-media.sndcdn.com/x.mp3");
  });

  it("refuses everything the app does not reach", () => {
    expect(targetOf(PROXY_PATH + enc("https://example.com/"))).toBeNull();
    expect(targetOf(PROXY_PATH + enc("http://api-v2.soundcloud.com/"))).toBeNull();
    // a service name inside another host's address is no service
    expect(targetOf(PROXY_PATH + enc("https://soundcloud.com.evil.example/"))).toBeNull();
    expect(targetOf(PROXY_PATH + enc("https://evil.example/?u=https://soundcloud.com/"))).toBeNull();
  });

  it("ignores other paths and broken ones", () => {
    expect(targetOf("/index.html")).toBeNull();
    expect(targetOf(PROXY_PATH + "%E0%A4%A")).toBeNull();
    expect(targetOf(PROXY_PATH)).toBeNull();
  });
});
