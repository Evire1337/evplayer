import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Track } from "@/types";
import { useLibrary } from "./store";
import { copyIsNewer, parseCopy, restoreLibraryCopy, startLibraryCopy } from "./copy";

const track = (id: string): Track => ({ id, source: "soundcloud", title: id, artists: [{ id: "a", name: "A" }], durationMs: 1000 });

beforeEach(() => useLibrary.setState({ likes: [], playlists: [], savedAt: undefined }));
afterEach(() => {
  vi.useRealTimers();
  delete (window as { evplayer?: unknown }).evplayer;
});

describe("library file copy", () => {
  it("reads a good copy and refuses a broken one", () => {
    expect(parseCopy(JSON.stringify({ savedAt: 5, likes: [], playlists: [] }))?.savedAt).toBe(5);
    expect(parseCopy("{nope")).toBeNull();
    expect(parseCopy(JSON.stringify({ likes: [] }))).toBeNull();
    expect(parseCopy(null)).toBeNull();
  });

  it("takes the copy only when it is newer than the storage", () => {
    const copy = { savedAt: 100, likes: [], playlists: [] };
    expect(copyIsNewer(copy, undefined)).toBe(true);
    expect(copyIsNewer(copy, 99)).toBe(true);
    expect(copyIsNewer(copy, 100)).toBe(false);
    expect(copyIsNewer(copy, 200)).toBe(false);
  });

  it("brings back a like the storage lost", async () => {
    const likes = [{ track: track("sc:1"), addedAt: "2026-01-01T00:00:00.000Z" }];
    const load = vi.fn().mockResolvedValue(JSON.stringify({ savedAt: 500, likes, playlists: [] }));
    (window as unknown as { evplayer: unknown }).evplayer = { libraryCopy: { load, save: vi.fn() } };
    useLibrary.setState({ likes: [], savedAt: 100 });
    await restoreLibraryCopy();
    expect(useLibrary.getState().likes.map((l) => l.track.id)).toEqual(["sc:1"]);
  });

  it("leaves a newer storage alone", async () => {
    const load = vi.fn().mockResolvedValue(JSON.stringify({ savedAt: 100, likes: [], playlists: [] }));
    (window as unknown as { evplayer: unknown }).evplayer = { libraryCopy: { load, save: vi.fn() } };
    useLibrary.setState({ likes: [{ track: track("sc:2"), addedAt: "x" }], savedAt: 900 });
    await restoreLibraryCopy();
    expect(useLibrary.getState().likes).toHaveLength(1);
  });

  it("writes the file a moment after a like and stamps the time once", () => {
    vi.useFakeTimers();
    const save = vi.fn().mockResolvedValue(undefined);
    (window as unknown as { evplayer: unknown }).evplayer = { libraryCopy: { load: vi.fn(), save } };
    startLibraryCopy();
    useLibrary.getState().toggleLike(track("sc:3"));
    expect(useLibrary.getState().savedAt).toBeGreaterThan(0);
    expect(save).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1100);
    expect(save).toHaveBeenCalledTimes(1);
    expect(JSON.parse(save.mock.calls[0][0]).likes).toHaveLength(1);
  });
});
