import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { StemSink } from "@/audio/graph";
import { CHUNK_FRAMES, SOURCE_RATE } from "./pcm";
import { StemPlayer, type PlayerHost } from "./player";

// The player against a stand-in deck, media server and sound node: what it asks for, when the
// deck goes silent, and above all that the deck is heard again whenever the tracks cannot play.

const TOTAL = CHUNK_FRAMES * 10;

class FakeDeck extends EventTarget {
  currentTime = 0;
  paused = false;
  seeking = false;
  readyState = 4;
  playbackRate = 1;
  preservesPitch = true;
}

type Sent = { type: string; [k: string]: unknown };

function setup(options: { status?: number; size?: () => number } = {}) {
  const deck = new FakeDeck();
  const sent: Sent[] = [];
  const muted: boolean[] = [];
  let reply: ((data: unknown) => void) | null = null;
  let contextNow = 0;
  const sink: StemSink = {
    port: {
      postMessage: (m: Sent) => void sent.push(m),
      set onmessage(fn: ((e: MessageEvent<unknown>) => void) | null) {
        reply = fn ? (data) => fn({ data } as MessageEvent<unknown>) : null;
      },
    } as unknown as MessagePort,
    setGains: vi.fn(),
    now: () => contextNow,
    dispose: vi.fn(),
  };
  const host: PlayerHost = {
    openStemSink: async () => sink,
    setDeckMuted: (_a, m) => void muted.push(m),
    setStemSpeed: vi.fn(),
  };
  const requests: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init: { headers: { Range: string } }) => {
      requests.push(`${url.split("/").pop()} ${init.headers.Range}`);
      const size = options.size ? options.size() : 44 + TOTAL * 4;
      const from = Number(/bytes=(\d+)-/.exec(init.headers.Range)?.[1] ?? 0);
      const status = options.status ?? (from >= size ? 416 : 206);
      return {
        status,
        ok: status < 300,
        headers: { get: () => `bytes 44-1/${size}` },
        arrayBuffer: async () => new ArrayBuffer(16),
        body: { cancel: async () => undefined },
      };
    }),
  );
  const player = new StemPlayer(host);
  const report = (frame: number, extra: Partial<{ playing: boolean; starved: boolean; at: number }> = {}) =>
    reply?.({ type: "pos", frame, at: extra.at ?? contextNow, playing: extra.playing ?? true, starved: extra.starved ?? false });
  return {
    deck,
    player,
    send: (data: unknown) => reply?.(data),
    sent,
    muted,
    requests,
    report,
    sink,
    engage: (failed: () => void = () => undefined, complete: () => boolean = () => true) =>
      player.engage(deck as unknown as HTMLAudioElement, "key", [1, 1, 1, 1], failed, complete),
    types: () => sent.map((m) => m.type),
  };
}

describe("StemPlayer", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    window.__EVPLAYER_MEDIA__ = "http://127.0.0.1:1/s";
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    delete window.__EVPLAYER_MEDIA__;
  });

  it("has nothing to play without a media server", () => {
    delete window.__EVPLAYER_MEDIA__;
    const t = setup();
    expect(t.engage()).toBe(false);
    expect(t.player.engaged).toBe(false);
  });

  it("asks for the chunks of all four tracks around the place, tells the node and starts it where the deck is", async () => {
    const t = setup();
    t.deck.currentTime = 20;
    expect(t.engage()).toBe(true);
    await vi.advanceTimersByTimeAsync(50);
    expect(new Set(t.requests.map((r) => r.split(" ")[0]))).toEqual(new Set(["vocals.wav", "drums.wav", "bass.wav", "other.wav"]));
    expect(t.types()).toContain("length");
    // one message per chunk (it carries the four tracks): the window, 5 to 9, is all there
    expect(t.sent.filter((m) => m.type === "chunk").map((m) => m.index).sort()).toEqual([5, 6, 7, 8, 9]);
    expect(t.sent.find((m) => m.type === "seek")?.frame).toBe(20 * SOURCE_RATE);
    expect(t.sent.filter((m) => m.type === "play").at(-1)?.on).toBe(true);
    // 20 s is chunk 6: one behind, the song's last chunk ahead
    expect(t.sent.filter((m) => m.type === "keep").at(-1)).toMatchObject({ from: 5, to: 9 });
  });

  it("keeps the deck heard until the tracks play in time, then silences it", async () => {
    const t = setup();
    t.engage();
    await vi.advanceTimersByTimeAsync(50);
    expect(t.muted).toEqual([]);
    t.deck.currentTime = 1;
    t.report(SOURCE_RATE);
    expect(t.muted).toEqual([]);
    t.report(SOURCE_RATE + 2048);
    expect(t.muted).toEqual([true]);
  });

  it("does not silence the deck while the tracks are far from it, and puts them back in step", async () => {
    const t = setup();
    t.engage();
    await vi.advanceTimersByTimeAsync(50);
    t.deck.currentTime = 10;
    const before = t.sent.filter((m) => m.type === "seek").length;
    await vi.advanceTimersByTimeAsync(500);
    t.report(5 * SOURCE_RATE);
    t.report(5 * SOURCE_RATE);
    await vi.advanceTimersByTimeAsync(50);
    expect(t.muted).toEqual([]);
    expect(t.sent.filter((m) => m.type === "seek").length).toBe(before + 1);
    expect(t.sent.filter((m) => m.type === "seek").at(-1)?.frame).toBe(10 * SOURCE_RATE);
  });

  it("gives the sound back to the deck when the tracks wait for data too long, and takes it again when they play", async () => {
    const t = setup();
    t.engage();
    await vi.advanceTimersByTimeAsync(50);
    t.report(0);
    t.report(2048);
    expect(t.muted).toEqual([true]);
    t.report(4096, { starved: true });
    await vi.advanceTimersByTimeAsync(700);
    t.report(4096, { starved: true });
    expect(t.muted).toEqual([true, false]);
    t.report(100);
    t.report(200);
    expect(t.muted).toEqual([true, false, true]);
  });

  it("gives the sound back to the deck when the node goes quiet", async () => {
    const t = setup();
    t.engage();
    await vi.advanceTimersByTimeAsync(50);
    t.report(0);
    t.report(2048);
    expect(t.muted).toEqual([true]);
    await vi.advanceTimersByTimeAsync(2500);
    expect(t.muted).toEqual([true, false]);
  });

  it("follows the deck: stops with it, moves with its seeks", async () => {
    const t = setup();
    t.engage();
    await vi.advanceTimersByTimeAsync(50);
    t.deck.paused = true;
    t.deck.dispatchEvent(new Event("pause"));
    expect(t.sent.at(-1)).toEqual({ type: "play", on: false });
    t.deck.currentTime = 25;
    t.deck.paused = false;
    t.deck.dispatchEvent(new Event("playing"));
    await vi.advanceTimersByTimeAsync(50);
    expect(t.sent.filter((m) => m.type === "seek").at(-1)?.frame).toBe(25 * SOURCE_RATE);
    expect(t.sent.filter((m) => m.type === "play").at(-1)).toEqual({ type: "play", on: true });
  });

  it("gives the sound back to the deck when the tracks end before the song does", async () => {
    const t = setup();
    t.engage();
    await vi.advanceTimersByTimeAsync(50);
    t.report(0);
    t.report(2048);
    expect(t.muted).toEqual([true]);
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    Object.assign(t.deck, { duration: 140, currentTime: 59 });
    t.send({ type: "end" });
    expect(t.muted).toEqual([true, false]);
    // at the very end of the song nothing is said: the deck ends by itself
    t.report(59 * SOURCE_RATE);
    t.report(59 * SOURCE_RATE + 2048);
    expect(t.muted).toEqual([true, false, true]);
    Object.assign(t.deck, { currentTime: 139.5 });
    t.send({ type: "end" });
    expect(t.muted).toEqual([true, false, true]);
    warn.mockRestore();
  });

  it("starts on a song that is still being split, takes only what is fully written and asks again for the rest", async () => {
    // the files hold 4.5 chunks now (the song is 10 chunks long and still being split)
    let written = 44 + Math.floor(CHUNK_FRAMES * 4.5) * 4;
    let done = false;
    const t = setup({ size: () => written });
    t.engage(() => undefined, () => done);
    await vi.advanceTimersByTimeAsync(50);
    const chunks = () => t.sent.filter((m) => m.type === "chunk").map((m) => m.index);
    // chunk 0 to 3 are all there; chunk 4 is half written and is not taken, and nothing after it is asked for
    expect(chunks().sort()).toEqual([0, 1, 2, 3]);
    expect(t.sent.some((m) => m.type === "length")).toBe(false);
    const asked = t.requests.length;
    await vi.advanceTimersByTimeAsync(100);
    t.report(0);
    expect(t.requests.length).toBe(asked);
    // more is written: it is taken after the wait, in order
    written = 44 + CHUNK_FRAMES * 6 * 4;
    await vi.advanceTimersByTimeAsync(400);
    t.report(0);
    await vi.advanceTimersByTimeAsync(50);
    expect(chunks().sort()).toEqual([0, 1, 2, 3, 4, 5]);
  });

  it("lets the deck be heard when a seek goes past what is split so far, and takes the sound again when it is there", async () => {
    let written = 44 + CHUNK_FRAMES * 3 * 4;
    const t = setup({ size: () => written });
    t.engage(() => undefined, () => false);
    await vi.advanceTimersByTimeAsync(50);
    t.report(0);
    t.report(2048);
    expect(t.muted).toEqual([true]);
    // the deck jumps to 10 s (chunk 3): not written yet
    t.deck.currentTime = 10;
    t.deck.dispatchEvent(new Event("seeking"));
    t.deck.dispatchEvent(new Event("seeked"));
    await vi.advanceTimersByTimeAsync(50);
    expect(t.muted).toEqual([true, false]);
    // it is written: the tracks follow and the deck goes silent again
    written = 44 + CHUNK_FRAMES * 8 * 4;
    await vi.advanceTimersByTimeAsync(400);
    t.report(10 * SOURCE_RATE);
    t.report(10 * SOURCE_RATE + 2048);
    expect(t.muted).toEqual([true, false, true]);
  });

  it("learns the length of the song when it is split to the end, and does not take the end as a missing chunk", async () => {
    const t = setup();
    let done = false;
    t.engage(() => undefined, () => done);
    await vi.advanceTimersByTimeAsync(50);
    done = true;
    t.report(0);
    await vi.advanceTimersByTimeAsync(50);
    expect(t.sent.filter((m) => m.type === "length").length).toBeGreaterThanOrEqual(1);
  });

  it("follows the speed", async () => {
    const t = setup();
    t.engage();
    await vi.advanceTimersByTimeAsync(50);
    t.deck.playbackRate = 1.5;
    t.player.updateSpeed();
    expect(t.sent.at(-1)).toEqual({ type: "rate", value: 1.5 });
  });

  it("gives up at once, and leaves the deck heard, when the tracks are not on disk", async () => {
    const t = setup({ status: 404 });
    const failed = vi.fn();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    t.engage(failed);
    await vi.advanceTimersByTimeAsync(100);
    expect(failed).toHaveBeenCalledTimes(1);
    expect(t.player.engaged).toBe(false);
    expect(t.muted.every((m) => m === false)).toBe(true);
    warn.mockRestore();
  });

  it("gives up if the tracks never start playing", async () => {
    const t = setup();
    const failed = vi.fn();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    t.engage(failed);
    await vi.advanceTimersByTimeAsync(7000);
    expect(failed).toHaveBeenCalledTimes(1);
    expect(t.muted).toEqual([false]);
    warn.mockRestore();
  });

  it("is let go cleanly: the deck is heard, the node is emptied, nothing is left listening", async () => {
    const t = setup();
    t.engage();
    await vi.advanceTimersByTimeAsync(50);
    t.report(0);
    t.report(2048);
    t.player.release(true);
    expect(t.muted.at(-1)).toBe(false);
    expect(t.sent.at(-1)).toEqual({ type: "reset" });
    expect(t.sink.dispose).toHaveBeenCalled();
    const count = t.sent.length;
    t.deck.dispatchEvent(new Event("pause"));
    t.report(4096);
    await vi.advanceTimersByTimeAsync(3000);
    expect(t.sent.length).toBe(count);
  });
});
