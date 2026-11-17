import { PARTS, driftAction, type Part } from "./model";

// The four tracks of a split song play on four elements that follow the song's own deck: the
// deck keeps the clock (play, pause, seek, speed all happen to it, as for any song) and the
// tracks are pulled along. Once all four really play, the deck itself goes silent, so what is
// heard is the four tracks through the same sound chain as everything else (speed, pitch,
// reverb, equalizer). If they do not start, the deck stays heard: a song is never left silent.
//
// The four elements (and their places in the sound graph) are made once and used for song
// after song; they are let go only when the switch is turned off.

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

/** how long the tracks get to start before they are given up on */
const START_WAIT_MS = 6000;

export class StemFollowers {
  private elements: HTMLAudioElement[] = [];
  private deck: HTMLAudioElement | null = null;
  private cleanup: Array<() => void> = [];
  private waiting: ReturnType<typeof setTimeout> | undefined;
  /** the deck is silent now (the four tracks are heard instead) */
  private silenced = false;
  /** all four tracks have played at least once */
  private started = false;
  /** since when (ms) a track has not been playing while the deck is silent; 0 = all well */
  private unwell = 0;

  constructor(private readonly host: FollowerHost) {}

  get engaged(): boolean {
    return this.deck !== null;
  }

  /** the four elements, made once; false if the graph would not take them */
  private ensureElements(): boolean {
    if (this.elements.length === PARTS.length) return true;
    const made = PARTS.map(() => {
      const el = new Audio();
      el.crossOrigin = "anonymous";
      el.preload = "auto";
      return el;
    });
    if (made.some((el) => this.host.addFollower(el) === null)) {
      this.host.dropFollowers();
      return false;
    }
    this.elements = made;
    return true;
  }

  /**
   * The tracks of the song `key` start following `deck`. `failed` is called if they do not start
   * (a file that is gone, a server that does not answer): the deck is heard again by then.
   */
  engage(deck: HTMLAudioElement, key: string, gains: number[], failed: () => void): boolean {
    this.release();
    const urls = PARTS.map((p) => stemUrl(key, p));
    if (urls.some((u) => u === null) || !this.ensureElements()) return false;

    this.deck = deck;
    this.host.setFollowerGains(gains);
    this.elements.forEach((el, i) => {
      el.src = urls[i] as string;
      el.load();
    });
    this.copyRate();
    this.seekAll();

    const listen = (target: HTMLElement, type: string, fn: () => void, once = false) => {
      target.addEventListener(type, fn, { once });
      this.cleanup.push(() => target.removeEventListener(type, fn));
    };
    listen(deck, "play", () => this.playAll());
    listen(deck, "pause", () => this.pauseAll());
    listen(deck, "seeked", () => this.seekAll());
    listen(deck, "ratechange", () => this.copyRate());
    listen(deck, "timeupdate", () => this.follow());
    listen(deck, "ended", () => this.pauseAll());

    const fail = () => {
      this.release();
      failed();
    };
    for (const el of this.elements) listen(el, "error", fail);

    // the deck goes silent once all four play; not before, and not at all if they never do
    this.started = false;
    this.silenced = false;
    this.unwell = 0;
    let playing = 0;
    for (const el of this.elements) {
      listen(el, "playing", () => {
        if (++playing === this.elements.length) {
          this.started = true;
          this.silenceDeck();
        }
      }, true);
    }
    // the clock for the tracks to start runs only while the song plays: a paused song waits
    const arm = () => {
      clearTimeout(this.waiting);
      this.waiting = setTimeout(() => {
        if (!this.started) {
          console.warn("[stems] the tracks did not start in time, the song plays as it is");
          fail();
        }
      }, START_WAIT_MS);
    };
    listen(deck, "play", () => {
      if (!this.started) arm();
    });
    if (!deck.paused) {
      arm();
      this.playAll();
    }
    return true;
  }

  setGains(gains: number[]): void {
    if (this.deck) this.host.setFollowerGains(gains);
  }

  /** the tracks stop following and are emptied; the deck is heard again. `drop` lets the elements go too */
  release(drop = false): void {
    clearTimeout(this.waiting);
    for (const off of this.cleanup) off();
    this.cleanup = [];
    if (this.deck) this.host.setDeckMuted(this.deck, false);
    this.deck = null;
    for (const el of this.elements) this.empty(el);
    if (drop) {
      this.elements = [];
      this.host.dropFollowers();
    }
  }

  private silenceDeck(): void {
    clearTimeout(this.waiting);
    if (this.deck) this.host.setDeckMuted(this.deck, true);
    this.silenced = true;
  }

  private unsilenceDeck(): void {
    if (this.deck) this.host.setDeckMuted(this.deck, false);
    this.silenced = false;
  }

  /** stops a track and lets its connection to the media server go */
  private empty(el: HTMLAudioElement): void {
    el.pause();
    if (el.getAttribute("src")) {
      el.removeAttribute("src");
      el.load();
    }
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

  /**
   * about four times a second: the tracks are looked after. One that stopped or ran dry for more
   * than a moment gives the sound back to the deck (a song is never left silent) and is started
   * again; once all play, the deck is silent again. One that has fallen behind or run ahead is
   * pulled back.
   */
  private follow(): void {
    const deck = this.deck;
    if (!deck || deck.paused) return;
    const well = this.elements.every((el) => !el.paused && el.readyState >= 3);
    if (this.started) {
      if (well) {
        this.unwell = 0;
        if (!this.silenced) this.silenceDeck();
      } else {
        this.unwell ||= performance.now();
        if (this.silenced && performance.now() - this.unwell > 1200) {
          console.warn("[stems] a track stopped: the deck is heard until they play again");
          this.unsilenceDeck();
          this.playAll();
        }
      }
    }
    for (const el of this.elements) {
      if (el.readyState < 2) continue;
      const action = driftAction(deck.currentTime, el.currentTime, deck.playbackRate);
      if (action.kind === "jump") el.currentTime = action.to;
      else if (Math.abs(el.playbackRate - action.rate) > 1e-6) el.playbackRate = action.rate;
    }
  }
}
