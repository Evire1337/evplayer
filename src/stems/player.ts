import type { StemSink } from "@/audio/graph";
import { PARTS, type Part } from "./model";
import { CHUNKS_TO_START, SOURCE_RATE, SYNC_OK_S, chunkBytes, chunkOf, framesFromContentRange, gapS, placeNow, windowAt } from "./pcm";

// The four tracks of a split song play from ONE node (public/worklets/stem-player.js) on the
// sound card's clock: they cannot drift apart or stall one by one. The song's own deck keeps the
// song's time (play, pause, seek, speed all happen to it, as for any song, and the progress bar
// and the lyrics read it), and the tracks follow it; once they play, the deck itself goes silent
// (its gate shuts), so what is heard is the four tracks through the same sound chain as
// everything else (levels, speed, pitch, reverb, equalizer).
//
// The sound is read from the media server in chunks of about 3 seconds, only around the place
// that plays, so memory stays small however long the song is. If anything goes wrong (a chunk
// does not come, the node says nothing) the deck is heard again at once: a song is never left
// silent.

/** what the player needs from the sound graph (audio/graph.ts) */
export interface PlayerHost {
  openStemSink: () => Promise<StemSink | null>;
  setDeckMuted: (audio: HTMLAudioElement, muted: boolean) => void;
  setStemSpeed: (rate: number, preservesPitch: boolean) => void;
}

/** where a track of a split song is served from; null where there is no media server */
export function stemUrl(key: string, part: Part): string | null {
  const base = typeof window !== "undefined" ? window.__EVPLAYER_MEDIA__ : undefined;
  return base ? `${base}/stem/${key}/${part}.wav` : null;
}

/** how long the tracks get to start playing before they are given up on */
const START_WAIT_MS = 6000;
/** reports in a row, in time with the deck, before the deck goes silent (about 80 ms) */
const HEALTHY_REPORTS = 2;
/** a track that waits for data this long gives the sound back to the deck */
const STARVED_GIVE_UP_MS = 500;
/** the node says nothing for this long while the deck is silent: the deck is heard again */
const SILENT_GIVE_UP_MS = 1500;
/** how often the tracks are put back in step with the deck at the most */
const RESYNC_MIN_MS = 400;
/** chunks that failed to come, one after another, before the tracks are given up on */
const MAX_FAILURES = 4;
/** a chunk that is not written yet (the song is still being split) is asked for again after this */
const NOT_YET_MS = 300;
const CONTENT_TOTAL = /\/(\d+)\s*$/;

interface Report {
  frame: number;
  at: number;
  playing: boolean;
  starved: boolean;
}

export class StemPlayer {
  private sink: StemSink | null = null;
  private deck: HTMLAudioElement | null = null;
  private key = "";
  private gains: number[] = [1, 1, 1, 1];
  private failed: () => void = () => undefined;
  /** the song is split to the end (until then its files are shorter than the song and still growing) */
  private isComplete: () => boolean = () => true;
  /** chunks not written yet, and when they are asked for again */
  private retryAt = new Map<number, number>();
  private probing = false;
  /** bumped whenever the song changes or is let go: what is still on its way for an old one is dropped */
  private gen = 0;
  private abort: AbortController | null = null;
  private cleanup: Array<() => void> = [];
  private timers: Array<ReturnType<typeof setInterval>> = [];
  private waiting: ReturnType<typeof setTimeout> | undefined;

  /** the song's length in frames, once a file has said */
  private total: number | null = null;
  /** chunks the node holds, and chunks on their way to it */
  private have = new Set<number>();
  private inflight = new Map<number, Promise<void>>();
  /** the window of chunks the node was told to keep */
  private kept = { from: 0, to: -1 };
  private failures = 0;
  /** the node has been told how long the song is */
  private lengthSent = false;

  private last: Report | null = null;
  private lastReportAt = 0;
  /** the deck is silent now (the tracks are heard instead) */
  private silenced = false;
  private started = false;
  private healthy = 0;
  private starvedSince = 0;
  private lastResync = 0;
  private alignToken = 0;

  constructor(private readonly host: PlayerHost) {}

  get engaged(): boolean {
    return this.deck !== null;
  }

  /**
   * The tracks of the song `key` start following `deck`. `failed` is called if they cannot play
   * (a file that is gone, a server that does not answer): the deck is heard again by then.
   * `complete` says whether the song is split to the end: the tracks can start while it is still
   * being split (the first seconds are written first), and what is not written yet is waited for.
   * False if there is nothing to play them from.
   */
  engage(deck: HTMLAudioElement, key: string, gains: number[], failed: () => void, complete: () => boolean = () => true): boolean {
    this.release();
    if (PARTS.some((p) => stemUrl(key, p) === null)) return false;
    const gen = ++this.gen;
    this.deck = deck;
    this.key = key;
    this.gains = gains;
    this.failed = failed;
    this.isComplete = complete;
    this.retryAt.clear();
    this.probing = false;
    this.abort = new AbortController();
    this.started = this.silenced = false;
    this.healthy = this.starvedSince = this.lastResync = this.failures = 0;
    this.lengthSent = false;
    this.last = null;
    this.lastReportAt = performance.now();

    const listen = (type: string, fn: () => void) => {
      deck.addEventListener(type, fn);
      this.cleanup.push(() => deck.removeEventListener(type, fn));
    };
    listen("play", () => {
      if (!this.started) this.arm();
    });
    // the deck really plays (not only asked to): the tracks are brought to its place
    listen("playing", () => void this.align());
    listen("pause", () => this.stop());
    listen("ended", () => this.stop());
    // a song that waits for its own stream waits for it with the tracks, or they would run ahead
    listen("waiting", () => this.stop());
    listen("seeking", () => this.stop());
    listen("seeked", () => void this.align());
    listen("ratechange", () => this.updateSpeed());
    // a safety net: a node that has gone quiet must not take the song with it
    this.timers.push(
      setInterval(() => {
        if (gen !== this.gen || !this.deck || this.deck.paused) return;
        if (this.silenced && performance.now() - this.lastReportAt > SILENT_GIVE_UP_MS) {
          console.warn("[stems] the player is silent: the deck is heard again");
          this.unsilenceDeck();
          void this.align();
        }
      }, 500),
    );
    if (!deck.paused) this.arm();
    void this.start(gen);
    return true;
  }

  setGains(gains: number[]): void {
    this.gains = gains;
    this.sink?.setGains(gains);
  }

  /** the speed or the link of speed and pitch changed: the tracks go the same way */
  updateSpeed(): void {
    const deck = this.deck;
    if (!deck) return;
    this.send({ type: "rate", value: deck.playbackRate });
    this.host.setStemSpeed(deck.playbackRate, deck.preservesPitch);
  }

  /** the tracks stop following and are let go; the deck is heard again. `drop` also takes the player out of the graph */
  release(drop = false): void {
    this.gen++;
    this.alignToken++;
    this.abort?.abort();
    this.abort = null;
    clearTimeout(this.waiting);
    for (const off of this.cleanup) off();
    this.cleanup = [];
    for (const t of this.timers) clearInterval(t);
    this.timers = [];
    if (this.deck) this.host.setDeckMuted(this.deck, false);
    this.deck = null;
    this.silenced = false;
    if (this.sink) {
      this.sink.port.postMessage({ type: "reset" });
      this.sink.port.onmessage = null;
      this.host.setStemSpeed(1, false);
      if (drop) {
        this.sink.dispose();
        this.sink = null;
      }
    }
    this.have.clear();
    this.inflight.clear();
    this.retryAt.clear();
    this.kept = { from: 0, to: -1 };
    this.total = null;
    this.last = null;
  }

  // ---- starting ------------------------------------------------------------------------

  private async start(gen: number): Promise<void> {
    try {
      const sink = this.sink ?? (await this.host.openStemSink());
      if (gen !== this.gen || !this.deck) return;
      if (!sink) throw new Error("the sound graph would not take the player");
      this.sink = sink;
      sink.port.onmessage = (event: MessageEvent<unknown>) => this.onMessage(event.data, gen);
      sink.setGains(this.gains);
      sink.port.postMessage({ type: "reset" });
      this.lengthSent = false;
      this.updateSpeed();
      await this.align();
    } catch (error) {
      if (gen === this.gen) this.giveUp(error);
    }
  }

  /** the tracks cannot play: the deck is heard again and the song is left as it is */
  private giveUp(reason: unknown): void {
    // once is enough: several chunks can fail at the same moment
    if (!this.deck) return;
    console.warn("[stems] the tracks cannot play, the song plays as it is:", reason);
    const failed = this.failed;
    this.release();
    failed();
  }

  /** the clock for the tracks to start runs only while the song plays: a paused song waits */
  private arm(): void {
    clearTimeout(this.waiting);
    const gen = this.gen;
    this.waiting = setTimeout(() => {
      if (gen === this.gen && !this.started) this.giveUp("they did not start in time");
    }, START_WAIT_MS);
  }

  // ---- the sound -----------------------------------------------------------------------

  private send(message: Record<string, unknown>): void {
    this.sink?.port.postMessage(message);
  }

  /** the tracks stop where they are (the deck paused, is seeking or waits for its stream) */
  private stop(): void {
    this.alignToken++;
    this.send({ type: "play", on: false });
  }

  /** the tracks are brought to the place the deck is at, and play if the deck does */
  private async align(): Promise<void> {
    const deck = this.deck;
    const gen = this.gen;
    if (!deck || !this.sink) return;
    const token = ++this.alignToken;
    this.lastResync = performance.now();
    try {
      await this.bring(deck.currentTime);
    } catch {
      // a chunk did not come: the deck stays heard (or is given back by the safety net) and it is tried again
      if (gen === this.gen && token === this.alignToken) setTimeout(() => gen === this.gen && token === this.alignToken && void this.align(), 700);
      return;
    }
    if (gen !== this.gen || token !== this.alignToken || !this.deck) return;
    // the time the chunks took is time the song went on
    let frame = Math.round(deck.currentTime * SOURCE_RATE);
    if (this.total !== null) frame = Math.min(frame, this.total);
    this.send({ type: "seek", frame });
    this.send({ type: "play", on: !deck.paused && !deck.seeking && deck.readyState >= 3 });
    this.healthy = 0;
    this.lastResync = performance.now();
  }

  // ---- the chunks ----------------------------------------------------------------------

  /** the chunks around `seconds` are in the node; the ones to start from are waited for */
  private async bring(seconds: number): Promise<void> {
    const frame = Math.max(0, seconds * SOURCE_RATE);
    const at = chunkOf(frame);
    this.moveWindow(frame);
    // the first one also tells how long the song is
    await this.fetchChunk(at);
    this.moveWindow(frame);
    const last = this.total === null ? at + CHUNKS_TO_START - 1 : chunkOf(Math.max(0, this.total - 1));
    const needed: Promise<void>[] = [];
    for (let i = at + 1; i < at + CHUNKS_TO_START && i <= last; i++) needed.push(this.fetchChunk(i));
    await Promise.all(needed);
    this.pump(frame);
  }

  /** the node keeps only the chunks near the place that plays */
  private moveWindow(frame: number): void {
    const w = windowAt(frame, this.total);
    if (w.from === this.kept.from && w.to === this.kept.to) return;
    this.kept = w;
    this.send({ type: "keep", from: w.from, to: w.to });
    for (const i of this.have) if (i < w.from || i > w.to) this.have.delete(i);
  }

  /** the chunks of the window that are not in the node yet are asked for (nothing waits for them here) */
  private pump(frame: number): void {
    this.moveWindow(frame);
    const now = performance.now();
    if (this.total === null && !this.probing && this.isComplete()) void this.probeTotal();
    for (let i = this.kept.from; i <= this.kept.to; i++) {
      // what is not written yet is not asked for, nor is anything after it
      if ((this.retryAt.get(i) ?? 0) > now) break;
      if (this.have.has(i) || this.inflight.has(i)) continue;
      this.fetchChunk(i).catch(() => undefined);
    }
  }

  /** once the song is split to the end, its length is the size of a file */
  private async probeTotal(): Promise<void> {
    this.probing = true;
    const gen = this.gen;
    try {
      const res = await fetch(stemUrl(this.key, "vocals") as string, { headers: { Range: "bytes=0-0" }, signal: this.abort?.signal });
      void res.body?.cancel();
      const total = framesFromContentRange(res.headers.get("Content-Range"));
      if (gen === this.gen && total !== null) {
        this.total = total;
        this.lengthSent = true;
        this.send({ type: "length", frames: total });
        this.moveWindow(this.last ? this.last.frame : 0);
      }
    } catch {
      // asked again with the next report
    } finally {
      if (gen === this.gen) this.probing = false;
    }
  }

  private fetchChunk(index: number): Promise<void> {
    const waiting = this.inflight.get(index);
    if (waiting) return waiting;
    if (this.have.has(index)) return Promise.resolve();
    const gen = this.gen;
    const key = this.key;
    const signal = this.abort?.signal;
    // asked before the files are fetched: a song that is whole by then has files that are final
    const finalFiles = this.isComplete();
    const job = (async () => {
      const { start, end } = chunkBytes(index);
      const responses = await Promise.all(
        PARTS.map((part) => fetch(stemUrl(key, part) as string, { headers: { Range: `bytes=${start}-${end}` }, signal })),
      );
      const notYet = () => {
        for (const r of responses) void r.body?.cancel();
        this.retryAt.set(index, performance.now() + NOT_YET_MS);
      };
      // past what is written: still being split (asked again soon), or past the end of the song
      if (responses.some((r) => r.status === 416)) {
        if (!finalFiles) notYet();
        else for (const r of responses) void r.body?.cancel();
        return;
      }
      const bad = responses.find((r) => !r.ok);
      if (bad) {
        for (const r of responses) void r.body?.cancel();
        // the files are gone (cleared, or never there): trying again will not bring them back
        if (bad.status === 404 && gen === this.gen) queueMicrotask(() => gen === this.gen && this.giveUp("the tracks are not on disk"));
        throw new Error(`the server answered ${bad.status}`);
      }
      // a chunk that is only partly written is not taken: it would play as a hole
      const size = Number(CONTENT_TOTAL.exec(responses[0].headers.get("Content-Range") ?? "")?.[1]);
      if (!finalFiles && !(size >= end + 1)) return notYet();
      if (this.total === null && finalFiles && gen === this.gen) this.total = framesFromContentRange(responses[0].headers.get("Content-Range"));
      const parts = await Promise.all(responses.map((r) => r.arrayBuffer()));
      // the song changed, or the place moved far away, while it came
      if (gen !== this.gen || index < this.kept.from || index > this.kept.to) return;
      if (this.total !== null && !this.lengthSent) {
        this.lengthSent = true;
        this.send({ type: "length", frames: this.total });
      }
      this.sink?.port.postMessage({ type: "chunk", index, parts }, parts);
      this.have.add(index);
      this.failures = 0;
    })()
      .catch((error: unknown) => {
        if (gen !== this.gen || (error instanceof DOMException && error.name === "AbortError")) return;
        if (++this.failures >= MAX_FAILURES) this.giveUp(error);
        else throw error;
      })
      .finally(() => {
        if (this.inflight.get(index) === job) this.inflight.delete(index);
      });
    this.inflight.set(index, job);
    return job;
  }

  // ---- looking after them --------------------------------------------------------------

  private onMessage(data: unknown, gen: number): void {
    if (gen !== this.gen || !this.deck || typeof data !== "object" || data === null) return;
    const message = data as { type?: string } & Partial<Report>;
    if (message.type === "end") {
      // the tracks are over but the song is not (they were made from a part of it): the deck is heard, never silence
      const deck = this.deck;
      if (!(deck.duration - deck.currentTime <= 1.5)) {
        console.warn("[stems] the tracks ended before the song: the deck is heard");
        this.unsilenceDeck();
      }
      return;
    }
    if (message.type !== "pos" || typeof message.frame !== "number") return;
    this.last = { frame: message.frame, at: message.at ?? 0, playing: !!message.playing, starved: !!message.starved };
    this.lastReportAt = performance.now();
    this.pump(message.frame);
    this.judge();
  }

  /** about 25 times a second: are the tracks heard in time, and is the deck still needed */
  private judge(): void {
    const deck = this.deck;
    const sink = this.sink;
    const report = this.last;
    if (!deck || !sink || !report || deck.paused || !report.playing) return;
    const now = performance.now();
    if (report.starved) {
      // waiting for data: the deck is heard if it takes long, and the tracks go on when it is there
      this.healthy = 0;
      this.starvedSince ||= now;
      if (this.silenced && now - this.starvedSince > STARVED_GIVE_UP_MS) {
        console.warn("[stems] the tracks wait for data: the deck is heard until they play again");
        this.unsilenceDeck();
      }
      return;
    }
    this.starvedSince = 0;
    const gap = gapS(placeNow(report.frame, report.at, sink.now(), deck.playbackRate), deck.currentTime);
    if (Math.abs(gap) > SYNC_OK_S) {
      this.healthy = 0;
      if (now - this.lastResync > RESYNC_MIN_MS) void this.align();
      return;
    }
    if (++this.healthy >= HEALTHY_REPORTS && !this.silenced) this.silenceDeck();
  }

  private silenceDeck(): void {
    clearTimeout(this.waiting);
    if (this.deck) this.host.setDeckMuted(this.deck, true);
    this.silenced = true;
    this.started = true;
  }

  private unsilenceDeck(): void {
    if (this.deck) this.host.setDeckMuted(this.deck, false);
    this.silenced = false;
    this.healthy = 0;
  }
}
