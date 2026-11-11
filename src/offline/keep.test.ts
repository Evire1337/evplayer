import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Track } from "@/types";
import type { OfflineEntry, OfflineSource } from "@/platform/api";

vi.mock("@/services/soundcloud/api", () => ({
  soundCloudStreams: vi.fn(async function* (id: string) {
    if (id === "sc:ogg") yield { url: "http://127.0.0.1/joined/x", hls: false, playlist: "https://cf-hls/ogg.m3u8" };
    else if (id === "sc:hls") yield { url: "https://cf-hls/a.m3u8", hls: true };
    else {
      yield { url: "https://cf-media/a.mp3", hls: false };
      yield { url: "https://cf-hls/b.m3u8", hls: true };
    }
  }),
}));
vi.mock("@/services/youtube/direct", () => ({
  resolveYouTubeAudio: vi.fn(async () => ({ url: "https://rr1.googlevideo.com/videoplayback?itag=251", contentLength: 2_500_000 })),
  videoIdOf: (id: string) => (/^yt:[A-Za-z0-9_-]{11}$/.test(id) ? id.slice(3) : null),
}));

import { cancelKeeping, forgetTracks, keepTracks, STALL_MS, watchJob, type KeepJob } from "./keep";
import { keptUrl, useKeepLists, useOffline } from "./store";

const track = (id: string, source: Track["source"] = "soundcloud"): Track => ({ id, source, title: id, artists: [{ id: "a", name: "A" }], durationMs: 1000 });
const entry = (id: string): OfflineEntry => ({ id, file: `${id}.mp3`, ext: "mp3", size: 1, savedAt: "now" });

let calls: Array<{ id: string; source: OfflineSource }>;
let download: (id: string, source: OfflineSource) => Promise<OfflineEntry>;

beforeEach(() => {
  calls = [];
  download = async (id) => entry(id);
  useOffline.setState({ entries: {}, progress: {}, loaded: true });
  useKeepLists.setState({ failed: [] });
  (window as unknown as { evplayer: unknown }).evplayer = {
    offline: {
      download: (id: string, source: OfflineSource) => {
        calls.push({ id, source });
        return download(id, source);
      },
      cancel: vi.fn(),
      remove: vi.fn(async () => undefined),
      mediaUrl: (id: string) => `http://127.0.0.1:1/s/offline/${id}`,
    },
  };
});
afterEach(() => cancelKeeping());

/** resolves when a batch that has started is over (the panel's job goes back to null) */
const idle = () => new Promise<void>((resolve) => {
  let started = false;
  const stop = watchJob((job) => {
    if (job) started = true;
    else if (started) {
      queueMicrotask(() => stop());
      resolve();
    }
  });
});

describe("keeping songs offline", () => {
  it("gives up on a song that stops answering, and goes on with the rest", async () => {
    vi.useFakeTimers();
    try {
      download = (id) => (id === "sc:hung" ? new Promise(() => undefined) : Promise.resolve(entry(id)));
      let last: KeepJob | null = null;
      const stop = watchJob((job) => {
        if (job) last = job;
      });
      expect(keepTracks([track("sc:hung"), track("sc:ok")])).toBe(2);
      await vi.advanceTimersByTimeAsync(STALL_MS + 10_000);
      stop();
      expect(last).toMatchObject({ total: 2, done: 2, failed: 1, reason: "сервис не отвечает" });
      expect(useOffline.getState().entries["sc:ok"]).toBeDefined();
      expect(useOffline.getState().entries["sc:hung"]).toBeUndefined();
    } finally {
      vi.useRealTimers();
    }
  });

  it("asks the desktop side for the right kind of source", async () => {
    const done = idle();
    expect(keepTracks([track("sc:1"), track("sc:hls"), track("sc:ogg"), track("yt:abcdefghijk", "youtube")])).toBe(4);
    await done;
    const by = Object.fromEntries(calls.map((c) => [c.id, c.source]));
    expect(by["sc:1"]).toEqual({ kind: "file", url: "https://cf-media/a.mp3" });
    expect(by["sc:hls"]).toEqual({ kind: "hls", url: "https://cf-hls/a.m3u8" });
    // a stream the app joins into one file is fetched from the playlist behind it
    expect(by["sc:ogg"]).toEqual({ kind: "hls", url: "https://cf-hls/ogg.m3u8" });
    // YouTube must say how long the song is
    expect(by["yt:abcdefghijk"]).toEqual({ kind: "file", url: "https://rr1.googlevideo.com/videoplayback?itag=251&clen=2500000" });
    expect(Object.keys(useOffline.getState().entries).sort()).toEqual(["sc:1", "sc:hls", "sc:ogg", "yt:abcdefghijk"]);
    expect(keptUrl("sc:1")).toBe("http://127.0.0.1:1/s/offline/sc:1");
    expect(keptUrl("sc:nope")).toBeNull();
  });

  it("skips what is kept, own files and repeats", async () => {
    useOffline.setState({ entries: { "sc:1": entry("sc:1") } });
    const done = idle();
    expect(keepTracks([track("sc:1"), track("local:5", "local"), track("sc:2"), track("sc:2")])).toBe(1);
    expect(keepTracks([track("sc:2")])).toBe(0);
    await done;
    expect(calls.map((c) => c.id)).toEqual(["sc:2"]);
  });

  it("tries the next format when one is refused, and reports a song that fails", async () => {
    download = async (id, source) => {
      if (source.url.endsWith("a.mp3")) throw new Error("protected");
      if (id === "sc:bad") throw new Error("всё отказано");
      return entry(id);
    };
    let last: KeepJob | null = null;
    const stop = watchJob((job) => job && (last = job));
    const done = idle();
    keepTracks([track("sc:2"), track("sc:bad")]);
    await done;
    stop();
    expect(calls.filter((c) => c.id === "sc:2").map((c) => c.source.url)).toEqual(["https://cf-media/a.mp3", "https://cf-hls/b.m3u8"]);
    expect(useOffline.getState().entries["sc:2"]).toBeDefined();
    expect(useOffline.getState().entries["sc:bad"]).toBeUndefined();
    expect(last).toMatchObject({ total: 2, done: 2, failed: 1 });
  });

  it("does not start a song that failed again on its own, only when asked", async () => {
    download = async (id) => {
      if (id === "sc:bad") throw new Error("всё отказано");
      return entry(id);
    };
    let done = idle();
    keepTracks([track("sc:bad")], { auto: true });
    await done;
    expect(useKeepLists.getState().failed).toEqual(["sc:bad"]);
    // the next start: the list tops itself up and leaves it out
    expect(keepTracks([track("sc:bad")], { auto: true })).toBe(0);
    // asked for by hand, it is tried again; once it works, it is no longer marked
    download = async (id) => entry(id);
    done = idle();
    expect(keepTracks([track("sc:bad")])).toBe(1);
    await done;
    expect(useKeepLists.getState().failed).toEqual([]);
  });

  it("never runs more than two at once and can be cancelled", async () => {
    let running = 0;
    let peak = 0;
    const gates: Array<() => void> = [];
    download = (id) =>
      new Promise((resolve) => {
        running++;
        peak = Math.max(peak, running);
        gates.push(() => {
          running--;
          resolve(entry(id));
        });
      });
    keepTracks(["sc:1", "sc:2", "sc:3", "sc:4"].map((id) => track(id)));
    await vi.waitFor(() => expect(gates.length).toBe(2));
    expect(peak).toBe(2);
    cancelKeeping();
    expect((window.evplayer!.offline.cancel as ReturnType<typeof vi.fn>).mock.calls.length).toBe(2);
    gates.forEach((open) => open());
    await new Promise((r) => setTimeout(r, 20));
    expect(calls.length).toBe(2); // the two waiting songs were dropped
  });

  it("forgets copies", async () => {
    useOffline.setState({ entries: { "sc:1": entry("sc:1"), "sc:2": entry("sc:2") } });
    await forgetTracks(["sc:1"]);
    expect(Object.keys(useOffline.getState().entries)).toEqual(["sc:2"]);
    expect(window.evplayer!.offline.remove).toHaveBeenCalledWith(["sc:1"]);
  });
});
