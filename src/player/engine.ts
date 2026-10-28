import Hls from "hls.js";
import type { Track } from "@/types";
import { currentTrack, nextIndex, prevIndex, shuffledQueue, usePlayer } from "./store";
import { resolveStream, streamCandidates, type PlayableStream } from "./streams";
import { mediaStream } from "@/platform/net";
import { createAudioGraph, type AudioGraph } from "@/audio/graph";
import { setLevelingTrack } from "@/equalizer/eqAudio";
import { effectsForNextTrack, useEffects, type EffectsState } from "./effects";
import { similarTracks, songKey } from "./radio";
import { keeps, useSettings } from "@/settings/store";
import { HISTORY_AFTER_MS, useHistory } from "@/stats/history";
import { currentSource, type PlaySource } from "@/stats/playSource";
import { recordListen } from "@/stats/listeningStats";

/** a source that has not started playing by then counts as not working */
const START_TIMEOUT_MS = 20_000;
/** a song that cannot be played gives way to the next one after this long */
export const SKIP_BROKEN_AFTER_MS = 500;
/** this many broken songs in a row stop the player (no internet, a service down) */
const MAX_FAILED_IN_A_ROW = 5;

// Two <audio> "decks": one plays, the other gets the next song ready shortly before the
// end, so it starts without a gap (or fades in over the end, with crossfade on). HLS streams
// go through hls.js, plain files play directly, YouTube songs play from their SoundCloud
// copy. The OS media controls are wired via the Media Session API.

interface Deck {
  audio: HTMLAudioElement;
  hls: Hls | null;
}

interface Prepared {
  index: number;
  track: Track;
  /** playing a copy of the song from this service */
  route?: Track["source"];
  ready: boolean;
}

/** how long before the end the next song starts loading */
const PREPARE_BEFORE_MS = 25_000;

function makeDeck(volume: number): Deck {
  const audio = new Audio();
  audio.crossOrigin = "anonymous";
  audio.preload = "auto";
  audio.volume = volume;
  return { audio, hls: null };
}

class PlayerEngine {
  private decks: [Deck, Deck];
  private active = 0;
  private loadAbort: AbortController | null = null;
  private loadToken = 0;
  private graph: AudioGraph;
  private firstTrack = true;
  // the next song, waiting on the other deck
  private prepared: Prepared | null = null;
  private prepareAbort: AbortController | null = null;
  private transitioning = false;
  // listening: how much of the current song was actually heard
  private session = "";
  private sessionTrack: Track | null = null;
  private heardMs = 0;
  private source: PlaySource | null = null;
  private failedInARow = 0;
  private prepareSkip: string | null = null;
  private fadeTimer: ReturnType<typeof setTimeout> | undefined;
  private failedToken = -1;
  private skipTimer: ReturnType<typeof setTimeout> | undefined;
  private sourceIds = new Set<string>();
  private lastPos = -1;
  private counted = false;
  private radioBusy = false;
  private playWhenExtended = false;
  /** a restored session starts at this position */
  private resumeAt = 0;

  private get cur(): Deck {
    return this.decks[this.active];
  }

  private get spare(): Deck {
    return this.decks[1 - this.active];
  }

  /** how far the heard sound is behind `audio.currentTime`, in ms */
  get outputLatencyMs(): number {
    return this.graph.latencyMs;
  }

  /** the element that is playing (for code that only needs to read it) */
  get audio(): HTMLAudioElement {
    return this.cur.audio;
  }

  constructor() {
    const state = usePlayer.getState();
    this.decks = [makeDeck(state.volume), makeDeck(state.volume)];
    for (const deck of this.decks) this.listen(deck);

    usePlayer.subscribe((s, prev) => {
      if (s.volume !== prev.volume || s.muted !== prev.muted) {
        for (const deck of this.decks) {
          deck.audio.volume = s.volume;
          deck.audio.muted = s.muted;
        }
      }
      // the queue changed under the prepared song: get the right one ready instead
      if (this.prepared && s.queue !== prev.queue && s.queue[this.prepared.index]?.id !== this.prepared.track.id) this.dropPrepared();
    });
    this.setupMediaSession();

    this.graph = createAudioGraph(this.decks.map((d) => d.audio));
    useEffects.subscribe((fx) => this.applyEffects(fx));
    window.addEventListener("beforeunload", () => this.finishListen());
  }

  private listen(deck: Deck): void {
    const a = deck.audio;
    const isCur = () => deck === this.cur;
    a.addEventListener("play", () => this.graph.resume());
    a.addEventListener("timeupdate", () => {
      if (!isCur()) return;
      const pos = a.currentTime * 1000;
      usePlayer.setState({ positionMs: pos });
      this.heard(pos, !a.paused);
      this.nearEnd(a);
    });
    a.addEventListener("durationchange", () => {
      if (isCur() && Number.isFinite(a.duration)) usePlayer.setState({ durationMs: a.duration * 1000 });
    });
    a.addEventListener("playing", () => {
      if (!isCur()) return;
      this.failedInARow = 0;
      usePlayer.setState({ status: "playing", error: undefined });
    });
    a.addEventListener("pause", () => {
      if (!isCur() || usePlayer.getState().status !== "playing") return;
      // paused from outside (a media key, the system) in the middle of a crossfade: the
      // fading-out song stops too, and the new one is simply paused
      if (this.transitioning) this.finishTransition();
      usePlayer.setState({ status: "paused" });
    });
    a.addEventListener("canplay", () => {
      if (deck === this.spare && this.prepared) this.prepared.ready = true;
    });
    a.addEventListener("ended", () => {
      if (!isCur()) return;
      if (this.prepared?.ready && usePlayer.getState().repeat !== "one") this.transition(0);
      else this.advance(false);
    });
    a.addEventListener("error", () => {
      if (deck === this.spare) {
        this.prepareFailed();
        return;
      }
      if (!isCur() || !a.getAttribute("src")) return;
      if (usePlayer.getState().status !== "loading") this.failed("Не удалось воспроизвести трек");
    });
  }

  private applyEffects(fx: EffectsState = useEffects.getState()): void {
    for (const { audio } of this.decks) {
      audio.defaultPlaybackRate = fx.speed;
      audio.playbackRate = fx.speed;
      // linked: the speed also moves the pitch, the way a faster record sounds higher
      audio.preservesPitch = !fx.linked;
    }
    this.graph.apply({ pitchSemitones: fx.linked ? 0 : fx.pitch, reverb: fx.reverb, reverbAmount: fx.reverbAmount });
  }

  // ---- listening ---------------------------------------------------------------------

  private heard(positionMs: number, playing: boolean): void {
    if (playing && this.lastPos >= 0) {
      const delta = positionMs - this.lastPos;
      // ignore seeks: only small forward steps count as listening
      if (delta > 0 && delta < 3000) this.heardMs += delta;
    }
    this.lastPos = positionMs;
    const track = this.sessionTrack;
    if (track && !this.counted && this.heardMs >= HISTORY_AFTER_MS) {
      this.counted = true;
      if (keeps("history")) {
        // songs added to the queue later are not part of the playlist it was started from
        const from = this.source && this.sourceIds.has(track.id) ? this.source : null;
        useHistory.getState().push(track, undefined, from);
      }
      recordListen(track, this.session, this.heardMs);
    }
  }

  private finishListen(): void {
    if (this.sessionTrack && this.counted) recordListen(this.sessionTrack, this.session, this.heardMs);
  }

  private startListen(track: Track): void {
    this.finishListen();
    this.session = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
    this.sessionTrack = track;
    this.heardMs = 0;
    this.lastPos = -1;
    this.counted = false;
  }

  // ---- queue ---------------------------------------------------------------------------

  /**
   * `source`: the playlist or album the queue comes from (by default the page on screen),
   * so it shows up in "Недавно слушали"
   */
  async playQueue(tracks: Track[], startIndex = 0, source: PlaySource | null = currentSource()): Promise<void> {
    if (tracks.length === 0) return;
    this.source = source;
    this.sourceIds = new Set(source ? tracks.map((t) => t.id) : []);
    this.resumeAt = 0;
    const { shuffle } = usePlayer.getState();
    const start = tracks[startIndex] ?? tracks[0];
    const queue = shuffle ? shuffledQueue(tracks, start.id) : tracks;
    const index = shuffle ? 0 : Math.max(0, startIndex);
    usePlayer.setState({ queue, index });
    await this.load(queue[index]);
  }

  /** last session's queue, shown paused until play is pressed */
  restore(queue: Track[], index: number, positionMs: number): void {
    const track = queue[index];
    usePlayer.setState({ queue, index, status: "idle", positionMs, durationMs: track.durationMs });
    this.resumeAt = positionMs > 3000 ? positionMs : 0;
    this.updateMetadata(track);
  }

  async playIndex(index: number): Promise<void> {
    const track = usePlayer.getState().queue[index];
    if (!track) return;
    this.resumeAt = 0;
    // "next" onto the song that is already waiting: switch at once
    if (this.prepared?.ready && this.prepared.index === index && this.prepared.track.id === track.id) {
      this.transition(0);
      return;
    }
    usePlayer.setState({ index });
    await this.load(track);
  }

  /** put a track right after the current one */
  playNext(track: Track): void {
    const s = usePlayer.getState();
    if (s.queue.length === 0) {
      void this.playQueue([track], 0, null);
      return;
    }
    const queue = [...s.queue];
    queue.splice(s.index + 1, 0, track);
    usePlayer.setState({ queue });
  }

  addToQueue(track: Track): void {
    const s = usePlayer.getState();
    if (s.queue.length === 0) {
      void this.playQueue([track], 0, null);
      return;
    }
    usePlayer.setState({ queue: [...s.queue, track] });
  }

  removeFromQueue(index: number): void {
    const s = usePlayer.getState();
    if (index === s.index || index < 0 || index >= s.queue.length) return;
    const queue = s.queue.filter((_, i) => i !== index);
    usePlayer.setState({ queue, index: index < s.index ? s.index - 1 : s.index });
  }

  moveInQueue(from: number, to: number): void {
    const s = usePlayer.getState();
    if (from === to || from < 0 || from >= s.queue.length) return;
    const queue = [...s.queue];
    const [item] = queue.splice(from, 1);
    const target = Math.max(0, Math.min(queue.length, to));
    queue.splice(target, 0, item);
    let index = s.index;
    if (from === s.index) index = target;
    else {
      if (from < s.index) index--;
      if (target <= index) index++;
    }
    usePlayer.setState({ queue, index });
  }

  clearUpcoming(): void {
    const s = usePlayer.getState();
    usePlayer.setState({ queue: s.queue.slice(0, s.index + 1) });
  }

  /** start a station from one song: the song, then similar ones */
  async startRadio(seed: Track): Promise<void> {
    await this.playQueue([seed], 0, null);
    const more = await similarTracks(seed, new Set([seed.id])).catch(() => []);
    const s = usePlayer.getState();
    if (currentTrack(s)?.id === seed.id) usePlayer.setState({ queue: [...s.queue, ...more] });
  }

  toggle(): void {
    const { status } = usePlayer.getState();
    if (status === "playing") this.pause();
    else if (status === "paused") this.resume();
    else if (status === "error" || status === "idle") {
      const track = currentTrack();
      if (track) void this.load(track);
    }
  }

  private pause(): void {
    // a crossfade in progress ends right here: the old song goes, the new one keeps its
    // full level and waits on pause (it used to stay half-done, and the player hung)
    if (this.transitioning) this.finishTransition();
    this.cur.audio.pause();
  }

  private resume(): void {
    void this.cur.audio.play().catch(() => {});
  }

  next(): void {
    this.advance(true);
  }

  prev(force = false): void {
    // like most players: a few seconds in, "previous" restarts the song
    if (!force && usePlayer.getState().positionMs > 4000) {
      this.seek(0);
      return;
    }
    const i = prevIndex(usePlayer.getState());
    if (i >= 0) void this.playIndex(i);
  }

  seek(ms: number): void {
    if (!Number.isFinite(ms)) return;
    const target = Math.max(0, ms);
    this.cur.audio.currentTime = target / 1000;
    this.lastPos = target;
    usePlayer.setState({ positionMs: target });
  }

  setVolume(volume: number): void {
    usePlayer.setState({ volume: Math.min(1, Math.max(0, volume)), muted: false });
  }

  toggleShuffle(): void {
    const state = usePlayer.getState();
    const track = currentTrack(state);
    if (!state.shuffle) {
      const queue = shuffledQueue(state.queue, track?.id);
      usePlayer.setState({ shuffle: true, queue, index: 0 });
    } else {
      usePlayer.setState({ shuffle: false });
    }
  }

  cycleRepeat(): void {
    const order = { off: "all", all: "one", one: "off" } as const;
    usePlayer.setState((s) => ({ repeat: order[s.repeat] }));
  }

  private advance(manual: boolean): void {
    const state = usePlayer.getState();
    const i = nextIndex(state, manual);
    if (i < 0) {
      if (useSettings.getState().autoplay) void this.extendWithSimilar(true);
      else if (manual && state.queue.length > 1) void this.playIndex(0);
      else usePlayer.setState({ status: "paused" });
      return;
    }
    if (i === state.index && !manual) {
      this.seek(0);
      this.resume();
      return;
    }
    void this.playIndex(i);
  }

  /**
   * Add similar songs after the queue. With `play`, jump to the first of them (the queue
   * ran out); without, it is a quiet top-up while the last song is still playing, so
   * "next" never has to wait.
   */
  private async extendWithSimilar(play: boolean): Promise<void> {
    const state = usePlayer.getState();
    const seed = currentTrack(state);
    if (!seed) return;
    if (this.radioBusy) {
      if (play) this.playWhenExtended = true;
      return;
    }
    this.radioBusy = true;
    if (play) this.playWhenExtended = true;
    try {
      // songs heard recently are not offered again straight away
      const recent = state.queue.slice(Math.max(0, state.index - 100));
      const more = await similarTracks(seed, new Set([...recent.map((t) => t.id), ...recent.map(songKey)]));
      const now = usePlayer.getState();
      if (!more.length) {
        if (this.playWhenExtended) usePlayer.setState({ status: "paused", error: "Не нашлось похожих треков" });
        return;
      }
      usePlayer.setState({ queue: [...now.queue, ...more] });
      if (this.playWhenExtended && now.index === state.index) await this.playIndex(state.index + 1);
    } catch {
      if (this.playWhenExtended) usePlayer.setState({ status: "paused" });
    } finally {
      this.radioBusy = false;
      this.playWhenExtended = false;
    }
  }

  /** what happens when a song starts, however it started */
  private begin(track: Track): void {
    this.startListen(track);
    this.updateMetadata(track);
    if (!this.firstTrack) useEffects.setState(effectsForNextTrack(useEffects.getState()));
    setLevelingTrack(track.id);
    this.firstTrack = false;
    const st = usePlayer.getState();
    if (st.index >= st.queue.length - 2 && st.repeat === "off" && useSettings.getState().autoplay) void this.extendWithSimilar(false);
  }

  private attach(deck: Deck, stream: { url: string; hls: boolean }, onFatal: () => void): void {
    if (stream.hls && Hls.isSupported()) {
      const hls = new Hls({ enableWorker: true, maxBufferLength: 30, backBufferLength: 30 });
      deck.hls = hls;
      hls.loadSource(stream.url);
      hls.attachMedia(deck.audio);
      hls.on(Hls.Events.ERROR, (_event, data) => data.fatal && onFatal());
    } else {
      // service addresses go through the app's proxy so the sound can pass the equalizer
      deck.audio.src = mediaStream(stream.url);
      deck.audio.load();
    }
  }

  private async load(track: Track): Promise<void> {
    const token = ++this.loadToken;
    this.loadAbort?.abort();
    const controller = new AbortController();
    this.loadAbort = controller;
    if (this.transitioning) this.finishTransition();
    this.dropPrepared();
    this.prepareSkip = null;
    this.unload(this.cur);
    usePlayer.setState({ status: "loading", positionMs: 0, durationMs: track.durationMs, error: undefined, route: undefined });
    this.begin(track);
    try {
      // one way after another (another format, the same song on the other service) until
      // one really plays
      let lastError: unknown = null;
      let started = false;
      for await (const stream of streamCandidates(track, controller.signal)) {
        if (token !== this.loadToken) return;
        try {
          await this.start(this.cur, stream, token);
        } catch (error) {
          if (token !== this.loadToken) return;
          if (error instanceof DOMException && error.name === "NotAllowedError") throw error;
          console.warn("[player] this source did not play, trying the next:", error);
          lastError = error;
          this.unload(this.cur);
          continue;
        }
        started = true;
        usePlayer.setState({ route: stream.via?.source });
        this.applyResume();
        break;
      }
      if (!started) throw lastError ?? new Error("Не нашлось, откуда взять звук этой песни");
    } catch (error) {
      if (token !== this.loadToken || controller.signal.aborted) return;
      // the system did not let it start on its own (no click yet): not the song's fault
      if (error instanceof DOMException && error.name === "NotAllowedError") {
        usePlayer.setState({ status: "paused" });
        return;
      }
      // the browser's own wording ("no supported source was found") means nothing to a listener
      const message = error instanceof DOMException && error.name === "NotSupportedError" ? "Не удалось открыть трек" : error instanceof Error ? error.message : String(error);
      this.failed(message);
    }
  }

  /** attach a source to a deck and wait until it really plays (or fails, or takes too long) */
  private start(deck: Deck, stream: PlayableStream, token: number): Promise<void> {
    return new Promise((resolve, reject) => {
      let settled = false;
      const settle = (error?: unknown) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        deck.audio.removeEventListener("error", onError);
        if (error) reject(error);
        else resolve();
      };
      const onError = () => settle(new Error("Не удалось открыть трек"));
      const timer = setTimeout(() => settle(new Error("Трек слишком долго не начинается")), START_TIMEOUT_MS);
      deck.audio.addEventListener("error", onError);
      this.graph.setDeck(deck.audio, 1);
      // a stream that breaks before it starts: the next source; after: the next song
      this.attach(deck, stream, () => {
        if (!settled) settle(new Error("Поток не открылся"));
        else if (token === this.loadToken) this.failed("Поток прервался");
      });
      this.applyEffects();
      deck.audio.play().then(() => settle(), settle);
    });
  }

  /**
   * The song on now cannot be played: say why and go on with the next one half a second
   * later instead of stopping. After several failures in a row (no internet, a service
   * down) the player stops rather than run through the whole queue.
   */
  private failed(message: string): void {
    const token = this.loadToken;
    // a broken stream can report itself twice (the audio element and the stream reader)
    if (this.failedToken === token) return;
    this.failedToken = token;
    this.failedInARow++;
    message = message.replace(/[.\s]+$/, "");
    if (this.failedInARow >= MAX_FAILED_IN_A_ROW) {
      this.failedInARow = 0;
      usePlayer.setState({ status: "error", error: `${message}. Подряд не включилось ${MAX_FAILED_IN_A_ROW} треков — проверьте интернет` });
      return;
    }
    usePlayer.setState({ status: "error", error: `${message} — включаю следующий` });
    clearTimeout(this.skipTimer);
    this.skipTimer = setTimeout(() => {
      if (token === this.loadToken && usePlayer.getState().status === "error") this.advance(true);
    }, SKIP_BROKEN_AFTER_MS);
  }

  // ---- the next song on the other deck -----------------------------------------------

  private nearEnd(audio: HTMLAudioElement): void {
    // a stream that does not tell its length: the length the service gave for the song
    const duration = Number.isFinite(audio.duration) && audio.duration > 0 ? audio.duration : usePlayer.getState().durationMs / 1000;
    if (!Number.isFinite(duration) || duration <= 0) return;
    const left = (duration - audio.currentTime) * 1000;
    const fade = useSettings.getState().crossfade;
    // the spare deck still holds the song fading out: prepare once the fade is over
    if (left < PREPARE_BEFORE_MS + fade * 1000 && !this.prepared && !this.transitioning) void this.prepareNext();
    if (fade > 0 && !this.transitioning && this.prepared?.ready && left <= fade * 1000 && usePlayer.getState().repeat !== "one") {
      this.transition(Math.min(fade, left / 1000));
    }
  }

  private async prepareNext(): Promise<void> {
    const state = usePlayer.getState();
    if (state.repeat === "one") return;
    const index = nextIndex(state, false);
    const track = state.queue[index];
    if (index < 0 || !track || index === state.index || track.id === this.prepareSkip) return;
    const prepared: Prepared = { index, track, ready: false };
    this.prepared = prepared;
    this.prepareAbort?.abort();
    const controller = new AbortController();
    this.prepareAbort = controller;
    try {
      const stream = await resolveStream(track, controller.signal);
      if (this.prepared !== prepared) return;
      if (stream.via) prepared.route = stream.via.source;
      const deck = this.spare;
      this.unload(deck);
      this.graph.setDeck(deck.audio, 0);
      this.attach(deck, stream, () => this.prepared === prepared && this.prepareFailed());
    } catch {
      if (this.prepared === prepared) this.prepared = null;
    }
  }

  /**
   * The next song's first source did not load on the spare deck. Not tried again before its
   * turn (it used to be retried on every tick): then it starts the full way, trying every
   * source.
   */
  private prepareFailed(): void {
    if (this.prepared) this.prepareSkip = this.prepared.track.id;
    this.dropPrepared();
  }

  private dropPrepared(): void {
    this.prepareAbort?.abort();
    this.prepareAbort = null;
    if (this.prepared && !this.transitioning) this.unload(this.spare);
    this.prepared = null;
  }

  /** hand over to the prepared song: at once (fade 0) or fading over `fade` seconds */
  private transition(fade: number): void {
    const prepared = this.prepared;
    if (!prepared) return;
    const state = usePlayer.getState();
    if (state.queue[prepared.index]?.id !== prepared.track.id) {
      this.dropPrepared();
      this.advance(false);
      return;
    }
    const old = this.cur;
    this.active = 1 - this.active;
    const now = this.cur;
    this.prepared = null;
    this.loadToken++;
    this.applyEffects();
    const smooth = fade > 0.2 && this.graph.canFade;
    if (smooth) {
      this.transitioning = true;
      this.graph.setDeck(now.audio, 0);
      this.graph.fadeDeck(now.audio, 1, fade);
      this.graph.fadeDeck(old.audio, 0, fade);
      clearTimeout(this.fadeTimer);
      this.fadeTimer = setTimeout(() => {
        if (this.transitioning && this.cur === now) this.finishTransition();
      }, fade * 1000 + 150);
    } else {
      this.graph.setDeck(now.audio, 1);
      this.unload(old);
    }
    usePlayer.setState({ index: prepared.index, status: "playing", positionMs: 0, durationMs: prepared.track.durationMs, error: undefined, route: prepared.route });
    this.begin(prepared.track);
    void now.audio.play().catch(() => usePlayer.setState({ status: "paused" }));
  }

  /** end a crossfade now: the old song is unloaded, the new one plays at full level */
  private finishTransition(): void {
    clearTimeout(this.fadeTimer);
    this.transitioning = false;
    this.graph.setDeck(this.cur.audio, 1);
    this.unload(this.spare);
  }

  private applyResume(): void {
    if (!this.resumeAt) return;
    const at = this.resumeAt;
    this.resumeAt = 0;
    this.seek(at);
  }

  private unload(deck: Deck): void {
    if (deck.hls) {
      deck.hls.destroy();
      deck.hls = null;
    }
    deck.audio.pause();
    if (deck.audio.getAttribute("src")) {
      deck.audio.removeAttribute("src");
      deck.audio.load();
    }
  }

  private updateMetadata(track: Track): void {
    if (!("mediaSession" in navigator)) return;
    navigator.mediaSession.metadata = new MediaMetadata({
      title: track.title,
      artist: track.artists.map((a) => a.name).join(", "),
      artwork: track.artwork && /^https:/.test(track.artwork) ? [{ src: track.artwork, sizes: "500x500" }] : [],
    });
  }

  private setupMediaSession(): void {
    if (!("mediaSession" in navigator)) return;
    const ms = navigator.mediaSession;
    ms.setActionHandler("play", () => this.toggle());
    ms.setActionHandler("pause", () => this.toggle());
    ms.setActionHandler("nexttrack", () => this.next());
    ms.setActionHandler("previoustrack", () => this.prev());
    ms.setActionHandler("seekto", (d) => d.seekTime !== undefined && this.seek(d.seekTime * 1000));
  }
}

export const player = new PlayerEngine();
