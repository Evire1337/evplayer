import { beforeEach, describe, expect, it } from "vitest";
import type { Track } from "@/types";
import { useLibrary } from "@/library/store";
import { LIKES_SCOPE, isKeepable, scopeStatus, scopeTracks, tracksOf } from "./scopes";

const track = (id: string, source: Track["source"] = "soundcloud"): Track => ({ id, source, title: id, artists: [{ id: "a", name: "A" }], durationMs: 1000 });

beforeEach(() => useLibrary.setState({ likes: [], playlists: [] }));

describe("offline scopes", () => {
  it("keeps only service songs, once each", () => {
    expect(isKeepable(track("sc:1"))).toBe(true);
    expect(isKeepable(track("yt:abcdefghijk", "youtube"))).toBe(true);
    expect(isKeepable(track("local:1", "local"))).toBe(false);
    const pl = useLibrary.getState().createPlaylist("Mix");
    useLibrary.getState().addToPlaylist(pl.id, [track("sc:1"), track("local:9", "local"), track("yt:abcdefghijk", "youtube")]);
    useLibrary.getState().addLikes([track("sc:1"), track("sc:2")]);
    expect(scopeTracks(pl.id).map((t) => t.id)).toEqual(["sc:1", "yt:abcdefghijk"]);
    expect(scopeTracks(LIKES_SCOPE).map((t) => t.id).sort()).toEqual(["sc:1", "sc:2"]);
    // a song in two lists is fetched once
    expect(tracksOf([pl.id, LIKES_SCOPE]).map((t) => t.id).sort()).toEqual(["sc:1", "sc:2", "yt:abcdefghijk"]);
  });

  it("counts what is already kept", () => {
    useLibrary.getState().addLikes([track("sc:1"), track("sc:2"), track("sc:3")]);
    expect(scopeStatus(LIKES_SCOPE, { "sc:2": true })).toEqual({ total: 3, kept: 1 });
    expect(scopeStatus("no-such-playlist", {})).toEqual({ total: 0, kept: 0 });
  });
});
