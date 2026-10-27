import { beforeEach, describe, expect, it } from "vitest";
import * as stats from "./listeningStats";

const track = (id: string, artist = "A") => ({ id, title: id, artists: [{ id: `ar_${artist}`, name: artist }], durationMs: 1000, source: "soundcloud" as const });

describe("listening stats", () => {
  beforeEach(() => {
    localStorage.clear();
    stats.resetListeningStatsForTests();
  });

  it("counts a session once and adds only the extra time", () => {
    const now = new Date(2026, 9, 26, 12);
    stats.recordListen(track("t"), "s1", 5000, now);
    stats.recordListen(track("t"), "s1", 60000, now);
    stats.recordListen(track("t"), "s2", 10000, now);
    stats.recordListen(track("t"), "s2", 25000, now);
    const summary = stats.getTrackStats("t", now);
    expect(summary.all).toEqual({ n: 2, ms: 85000 });
    expect(summary.daily.at(-1)?.n).toBe(2);
  });

  it("splits week, month and all-time", () => {
    const now = new Date(2026, 9, 26, 12);
    stats.recordListen(track("t"), "a", 1000, new Date(2026, 9, 25));
    stats.recordListen(track("t"), "b", 1000, new Date(2026, 9, 10));
    stats.recordListen(track("t"), "c", 1000, new Date(2026, 5, 1));
    const summary = stats.getTrackStats("t", now);
    expect([summary.week.n, summary.month.n, summary.all.n]).toEqual([1, 2, 3]);
  });

  it("ranks tracks and artists", () => {
    const now = new Date(2026, 9, 26);
    stats.recordListen(track("x", "B"), "1", 1000, now);
    stats.recordListen(track("y", "C"), "2", 1000, now);
    stats.recordListen(track("y", "C"), "3", 1000, now);
    expect(stats.getTopTracks("week", 10, now).map((r) => r.track.id)).toEqual(["y", "x"]);
    expect(stats.getTopArtists("all", 10, now)[0]).toMatchObject({ name: "C", plays: 2 });
  });

  it("survives a reload", () => {
    stats.recordListen(track("t"), "s", 1000);
    stats.flushListeningStats();
    stats.resetListeningStatsForTests();
    expect(stats.getTrackStats("t").all.n).toBe(1);
  });

  it("builds smart playlists", () => {
    const now = new Date(2026, 9, 26);
    const day = (d: number) => new Date(2026, 9, 26 - d);
    for (let i = 0; i < 3; i++) stats.recordListen(track("repeat"), `r${i}`, 1000, day(i));
    for (let i = 0; i < 4; i++) stats.recordListen(track("old"), `o${i}`, 1000, day(100 + i));
    stats.recordListen(track("fresh"), "f", 1000, day(2));
    expect(stats.getSmartPlaylist("on_repeat", [], now).map((t) => t.id)).toEqual(["repeat"]);
    expect(stats.getSmartPlaylist("forgotten", [], now).map((t) => t.id)).toEqual(["old"]);
    expect(stats.getSmartPlaylist("discoveries", [], now).map((t) => t.id).sort()).toEqual(["fresh", "repeat"]);
    expect(stats.getSmartPlaylist("liked_rare", [track("fresh"), track("never"), track("repeat")], now).map((t) => t.id)).toEqual(["fresh", "never"]);
  });
});
