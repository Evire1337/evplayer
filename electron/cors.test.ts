// @vitest-environment node
import { describe, expect, it } from "vitest";
import { isOpenHost, openCorsHeaders } from "./cors";

describe("cors", () => {
  it("opens only the music hosts", () => {
    expect(isOpenHost("https://api-v2.soundcloud.com/search")).toBe(true);
    expect(isOpenHost("https://cf-hls-media.sndcdn.com/x")).toBe(true);
    expect(isOpenHost("https://evil.example.com/soundcloud.com")).toBe(false);
  });
  it("replaces the service's cors headers with open ones", () => {
    const out = openCorsHeaders({ "Access-Control-Allow-Origin": ["https://soundcloud.com"], "Content-Type": ["application/json"] });
    expect(out["access-control-allow-origin"]).toEqual(["*"]);
    expect(out["Content-Type"]).toEqual(["application/json"]);
    expect(out["Access-Control-Allow-Origin"]).toBeUndefined();
  });
});
