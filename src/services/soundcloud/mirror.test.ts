import { describe, expect, it } from "vitest";
import type { Playlist } from "@/library/store";
import type { Track } from "@/types";
import { plan, scTrackId, signatureOf, trackIds } from "./mirror";

const track = (id: string): Track => ({ id, source: id.startsWith("sc:") ? "soundcloud" : "youtube", title: id, artists: [], durationMs: 1000 }) as Track;
const like = (id: string, at: string) => ({ track: track(id), addedAt: at });
const list = (id: string, ids: string[], extra: Partial<Playlist> = {}): Playlist => ({
  id,
  title: "Mix",
  createdAt: "",
  updatedAt: "",
  items: ids.map((t, i) => ({ id: `i${i}`, track: track(t), addedAt: "" })),
  ...extra,
});
const empty = { likes: [], playlists: {} };

describe("soundcloud mirror plan", () => {
  it("only SoundCloud songs count", () => {
    expect(scTrackId("sc:42")).toBe("42");
    expect(scTrackId("yt:abc")).toBeNull();
    expect(trackIds(list("a", ["sc:1", "yt:x", "sc:1", "sc:2"]))).toEqual(["1", "2"]);
  });

  it("likes the new songs oldest first and unlikes the removed ones", () => {
    const steps = plan([like("sc:3", "2024-03"), like("sc:1", "2024-01"), like("yt:z", "2024-02"), like("sc:2", "2024-02")], [], { likes: ["9"], playlists: {} });
    expect(steps).toEqual([
      { kind: "like", track: "1" },
      { kind: "like", track: "2" },
      { kind: "like", track: "3" },
      { kind: "unlike", track: "9" },
    ]);
  });

  it("does nothing when the library matches the baseline", () => {
    const p = list("p", ["sc:1", "sc:2"]);
    expect(plan([like("sc:1", "a")], [p], { likes: ["1"], playlists: { p: { scId: "77", signature: signatureOf(p) } } })).toEqual([]);
  });

  it("creates, updates (also by order) and drops playlists", () => {
    const a = list("a", ["sc:1", "sc:2"]);
    const reordered = list("a", ["sc:2", "sc:1"]);
    const fresh = list("b", ["sc:5"]);
    const base = { likes: [], playlists: { a: { scId: "10", signature: signatureOf(a) }, gone: { scId: "11", signature: "x" } } };
    expect(plan([], [reordered, fresh], base)).toEqual([
      { kind: "update", playlist: "a", scId: "10" },
      { kind: "create", playlist: "b" },
      { kind: "drop", playlist: "gone", scId: "11" },
    ]);
  });

  it("leaves playlists imported from SoundCloud alone", () => {
    expect(plan([], [list("i", ["sc:1"], { source: "scp:5" })], empty)).toEqual([]);
  });

  it("a renamed playlist is an update", () => {
    const p = list("p", ["sc:1"]);
    const base = { likes: [], playlists: { p: { scId: "1", signature: signatureOf(p) } } };
    expect(plan([], [{ ...p, title: "New" }], base)).toHaveLength(1);
  });
});
