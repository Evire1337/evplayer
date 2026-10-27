import { describe, expect, it } from "vitest";
import { activityFor, decodeFrames, encodeFrame } from "./discord";

describe("discord frames", () => {
  it("round-trips frames and keeps partial data", () => {
    const a = encodeFrame(1, { evt: "READY" });
    const b = encodeFrame(1, { x: 2 });
    const { frames, rest } = decodeFrames(Buffer.concat([a, b.subarray(0, 5)]));
    expect(frames).toEqual([{ op: 1, data: { evt: "READY" } }]);
    expect(rest.length).toBe(5);
  });
  it("builds a listening activity", () => {
    const act = activityFor({ title: "A", artist: "B", startedAt: 1, endsAt: 2, artwork: "https://x/y.jpg" });
    expect(act.type).toBe(2);
    expect(act.details).toBe("A  ");
    expect(act.timestamps).toEqual({ start: 1, end: 2 });
    expect(act.assets.large_image).toBe("https://x/y.jpg");
  });
});
