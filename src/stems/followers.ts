import { PARTS, driftAction, type Part } from "./model";

// The four tracks of a split song play on four elements that follow the song's own deck: the
// deck keeps the clock (play, pause, seek, speed all happen to it, as for any song) and the
// tracks are pulled along. While they play, the deck itself is silent, so what is heard is the
// four tracks through the same sound chain as everything else (speed, pitch, reverb, equalizer).

/** what the followers need from the sound graph (audio/graph.ts) */
export interface FollowerHost {
  addFollower: (audio: HTMLAudioElement) => unknown;
  setFollowerGains: (values: number[]) => void;
  dropFollowers: () => void;
  setDeckMuted: (audio: HTMLAudioElement, muted: boolean) => void;
}

/** where a track of a split song is served from; null where there is no media server */
export function stemUrl(key: string, part: Part): string | null {
  const base = typeof window !== "undefined" ? window.__EVPLAYER_MEDIA__ : undefined;
  return base ? `${base}/stem/${key}/${part}.wav` : null;
}

/** how long the deck waits for the tracks to start before it is heard again */
const START_WAIT_MS = 4000;

export class StemFollowers {
  private elements: HTMLAudioElement[] = [];
  private deck: HTMLAudioElement | null = null;
  private listeners: Array<() => void> = [];
  private waiting: ReturnType<typeof setTimeout> | undefined;

  constructor(private readonly host: FollowerHost) {}

  get engaged(): boolean {
    return this.deck !== null;
  }

  /** the tracks of the song `key` start following `deck`; false if they cannot (no server, no graph) */
  engage(deck: HTMLAudioElement, key: string, gains: number[]): boolean {
    this.release();
    const urls = PARTS.map((p) => stemUrl(key, p));
    if (urls.some((u) => u === null)) return false;
    const elements = urls.map((url) => {
      const el = new Audio();
      el.crossOrigin = "anonymous";
      el.preload = "auto";
      el.src = url as string;
      return el;
    });
    // a track the graph would not take is no use: let everything go
    if (elements.some((el) => this.host.addFollower(el) === null)) {
      for (const el of elements) this.drop(el);
      this.host.dropFollowers();
      return false;
    }
    this.elements = elements;
    this.deck = deck;
    this.host.setFollowerGains(gains);
    this.copyRate();
    this.seekAll();

    const on = (type: string, fn: () => void) => {
      deck.addEventListener(type, fn);
      this.listeners.push(() => deck.removeEventListener(type, fn));
    };
    on("play", () => this.playAll());
    on("pause", () => this.pauseAll());
    on("seeked", () => this.seekAll());
    on("ratechange", () => this.copyRate());
    on("timeupdate", () => this.follow());
    on("ended", () => this.pauseAll());

    // heard from the tracks once all four play; the deck stays silent from then on
    let playing = 0;
    const heard = () => {
      if (++playing === elements.length) this.silenceDeck();
    };
    for (const el of elements) el.addEventListener("playing", heard, { once: true });
    this.waiting = setTimeout(() => this.silenceDeck(), START_WAIT_MS);
    if (!deck.paused) this.playAll();
    return true;
  }

  setGains(gains: number[]): void {
    if (this.deck) this.host.setFollowerGains(gains);
  }

  /** the tracks stop following; the deck is heard again. `drop` also lets the elements go */
  release(drop = false): void {
    clearTimeout(this.waiting);
    for (const off of this.listeners) off();
    this.listeners = [];
    if (this.deck) this.host.setDeckMuted(this.deck, false);
    this.deck = null;
    for (const el of this.elements) this.drop(el);
    this.elements = [];
    if (drop) this.host.dropFollowers();
  }

  private silenceDeck(): void {
    clearTimeout(this.waiting);
    if (this.deck) this.host.setDeckMuted(this.deck, true);
  }

  private drop(el: HTMLAudioElement): void {
    el.pause();
    el.removeAttribute("src");
    el.load();
  }

  private playAll(): void {
    for (const el of this.elements) void el.play().catch(() => undefined);
  }

  private pauseAll(): void {
    for (const el of this.elements) el.pause();
  }

  private seekAll(): void {
    if (!this.deck) return;
    for (const el of this.elements) {
      try {
        el.currentTime = this.deck.currentTime;
      } catch {
        // not loaded yet: it starts at the beginning and is pulled along
      }
    }
  }

  private copyRate(): void {
    if (!this.deck) return;
    for (const el of this.elements) {
      el.defaultPlaybackRate = this.deck.playbackRate;
      el.playbackRate = this.deck.playbackRate;
      el.preservesPitch = this.deck.preservesPitch;
    }
  }

  /** about four times a second: a track that has fallen behind or run ahead is pulled back */
  private follow(): void {
    const deck = this.deck;
    if (!deck || deck.paused) return;
    for (const el of this.elements) {
      if (el.readyState < 2) continue;
      const action = driftAction(deck.currentTime, el.currentTime, deck.playbackRate);
      if (action.kind === "jump") el.currentTime = action.to;
      else if (Math.abs(el.playbackRate - action.rate) > 1e-6) el.playbackRate = action.rate;
    }
  }
}

