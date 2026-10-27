import { describe, expect, it } from "vitest";
import type { Track } from "@/types";
import { nextIndex, prevIndex, shuffledQueue, type PlayerState } from "./store";

const t = (id: string): Track => ({ id, source: "soundcloud", title: id, artists: [], durationMs: 1 });
const base = (over: Partial<PlayerState>): PlayerState => ({
  queue: [t("a"), t("b"), t("c")],
  index: 0,
  status: "playing",
  positionMs: 0,
  durationMs: 0,
  volume: 1,
  muted: false,
  repeat: "off",
  shuffle: false,
  ...over,
});

describe("queue navigation", () => {
  it("stops at the end unless repeating", () => {
    expect(nextIndex(base({ index: 2 }), false)).toBe(-1);
    expect(nextIndex(base({ index: 2, repeat: "all" }), false)).toBe(0);
    expect(nextIndex(base({ index: 1, repeat: "one" }), false)).toBe(1);
    expect(nextIndex(base({ index: 1, repeat: "one" }), true)).toBe(2);
    // "next" on the last song does not start the queue over: autoplay continues instead
    expect(nextIndex(base({ index: 2 }), true)).toBe(-1);
    expect(prevIndex(base({ index: 0, repeat: "all" }))).toBe(2);
  });
  it("shuffles but keeps the chosen track first", () => {
    const q = shuffledQueue([t("a"), t("b"), t("c"), t("d")], "c");
    expect(q[0].id).toBe("c");
    expect(q.map((x) => x.id).sort()).toEqual(["a", "b", "c", "d"]);
  });
});
