import { describe, expect, it } from "vitest";
import { pushEntry } from "./history";

const t = (id: string) => ({ id, source: "soundcloud" as const, title: id, artists: [], durationMs: 1 });

describe("history", () => {
  it("puts the newest first and keeps one entry per track", () => {
    let list = pushEntry([], t("a"), new Date(1));
    list = pushEntry(list, t("b"), new Date(2));
    list = pushEntry(list, t("a"), new Date(3));
    expect(list.map((e) => e.track.id)).toEqual(["a", "b"]);
  });
});
