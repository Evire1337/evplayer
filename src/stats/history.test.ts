import { describe, expect, it } from "vitest";
import { pushEntry, pushSourceEntry } from "./history";

const t = (id: string) => ({ id, source: "soundcloud" as const, title: id, artists: [], durationMs: 1 });

describe("history", () => {
  it("puts the newest first and keeps one entry per track", () => {
    let list = pushEntry([], t("a"), new Date(1));
    list = pushEntry(list, t("b"), new Date(2));
    list = pushEntry(list, t("a"), new Date(3));
    expect(list.map((e) => e.track.id)).toEqual(["a", "b"]);
  });
});

describe("recently played playlists", () => {
  it("marks songs played from a playlist", () => {
    expect(pushEntry([], t("a"), new Date(1), { page: "playlist", id: "p1" })[0].from).toBe("playlist:p1");
    expect(pushEntry([], t("b"), new Date(1))[0].from).toBeUndefined();
  });

  it("keeps one card per playlist, newest first", () => {
    let s = pushSourceEntry([], { page: "playlist", id: "p1" }, new Date(1));
    s = pushSourceEntry(s, { page: "collection", id: "scp:1", title: "Album" }, new Date(2));
    s = pushSourceEntry(s, { page: "playlist", id: "p1" }, new Date(3));
    expect(s.map((e) => e.source.id)).toEqual(["p1", "scp:1"]);
  });
});
