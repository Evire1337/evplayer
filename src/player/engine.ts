import Hls from "hls.js";
import type { Track } from "@/types";
import { currentTrack, nextIndex, prevIndex, shuffledQueue, usePlayer } from "./store";
import { resolveStream } from "./streams";
import { createAudioGraph, type AudioGraph } from "@/audio/graph";
import { setLevelingTrack } from "@/equalizer/eqAudio";
import { effectsForNextTrack, useEffects, type EffectsState } from "./effects";
import { YouTubeEmbed } from "./youtubeEmbed";
import { similarTracks, songKey } from "./radio";
import { useSettings } from "@/settings/store";
import { HISTORY_AFTER_MS, useHistory } from "@/stats/history";
import { recordListen } from "@/stats/listeningStats";

// Two <audio> "decks": one plays, the other gets the next song ready shortly before the
// end, so it starts without a gap (or fades in over the end, with crossfade on). HLS streams
// go through hls.js, plain files play directly, YouTube songs without a SoundCloud copy use
// the official YouTube player. The OS media controls are wired via the Media Session API.

type Mode = "audio" | "embed";

interface Deck {
  audio: HTMLAudioElement;
  hls: Hls | null;
}

interface Prepared {
  index: number;
  track: Track;
  route?: "matched";
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
  private mode: Mode = "audio";
  private embed: YouTubeEmbed;
  // the next song, waiting on the other deck
  private prepared: Prepared | null = null;
  private prepareAbort: AbortController | null = null;
  private transitioning = false;
  // listening: how much of the current song was actually heard
  private session = "";
  private sessionTrack: Track | null = null;
  private heardMs = 0;
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

  /** the element that is playing (for code that only needs to read it) */
  get audio(): HTMLAudioElement {
    return this.cur.audio;
  }

  constructor() {
    const state = usePlayer.getState();
    this.decks = [makeDeck(state.volume), makeDeck(state.volume)];
    for (const deck of this.decks) this.listen(deck);

    this.embed = new YouTubeEmbed({
      onPlaying: () => usePlayer.setState({ status: "playing", error: undefined }),
      onPaused: () => usePlayer.setState({ status: "paused" }),
      onBuffering: () => {},
      onEnded: () => this.advance(false),
      onTime: (pos, duration) => {
        usePlayer.setState({ positionMs: pos, ...(duration ? { durationMs: duration } : {}) });
        this.heard(pos, usePlayer.getState().status === "playing");
      },
      onError: (message) => {
        this.embed.stop();
        usePlayer.setState({ status: "error", error: message });
        // a video that may not be embedded: go on with the next song instead of stopping
        const token = this.loadToken;
        const s = usePlayer.getState();
        if (s.index < s.queue.length - 1) setTimeout(() => token === this.loadToken && this.advance(true), 2500);
      },
    });

    usePlayer.subscribe((s, prev) => {
      if (s.volume !== prev.volume || s.muted !== prev.muted) {
        for (const deck of this.decks) {
          deck.audio.volume = s.volume;
          deck.audio.muted = s.muted;
        }
        this.embed.setVolume(s.volume, s.muted);
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
    const isCur = () => deck === this.cur && this.mode === "audio";
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
    a.addEventListener("playing", () => isCur() && usePlayer.setState({ status: "playing", error: undefined }));
    a.addEventListener("pause", () => {
      if (isCur() && !this.transitioning && usePlayer.getState().status === "playing") usePlayer.setState({ status: "paused" });
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
        this.dropPrepared();
        return;
      }
      if (!isCur() || !a.getAttribute("src")) return;
      if (usePlayer.getState().status !== "loading") usePlayer.setState({ status: "error", error: "Не удалось воспроизвести трек" });
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
    if (this.mode === "embed") this.embed.setRate(fx.speed);
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
      useHistory.getState().push(track);
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

  async playQueue(tracks: Track[], startIndex = 0): Promise<void> {
    if (tracks.length === 0) return;
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
    if (this.prepared?.ready && this.prepared.index === index && this.prepared.track.id === track.id && this.mode === "audio") {
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
      void this.playQueue([track]);
      return;
    }
    const queue = [...s.queue];
    queue.splice(s.index + 1, 0, track);
    usePlayer.setState({ queue });
  }

  addToQueue(track: Track): void {
    const s = usePlayer.getState();
    if (s.queue.length === 0) {
      void this.playQueue([track]);
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
    await this.playQueue([seed]);
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
    if (this.mode === "embed") this.embed.pause();
    else {
      this.cur.audio.pause();
      // a crossfade in progress stops with it
      if (this.transitioning) this.spare.audio.pause();
    }
  }

  private resume(): void {
    if (this.mode === "embed") this.embed.play();
    else void this.cur.audio.play().catch(() => {});
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
    if (this.mode === "embed") this.embed.seek(target);
    else this.cur.audio.currentTime = target / 1000;
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
      deck.audio.src = stream.url;
      deck.audio.load();
    }
  }

  private async load(track: Track): Promise<void> {
    const token = ++this.loadToken;
    this.loadAbort?.abort();
    const controller = new AbortController();
    this.loadAbort = controller;
    this.dropPrepared();
    this.transitioning = false;
    this.unload(this.cur);
    usePlayer.setState({ status: "loading", positionMs: 0, durationMs: track.durationMs, error: undefined, route: undefined });
    this.begin(track);
    try {
      const stream = await resolveStream(track, controller.signal);
      if (token !== this.loadToken) return;
      if (stream.kind === "embed") {
        this.mode = "embed";
        usePlayer.setState({ route: "embed" });
        const { volume, muted } = usePlayer.getState();
        await this.embed.load(stream.videoId, volume, muted);
        this.applyResume();
        return;
      }
      this.mode = "audio";
      this.embed.stop();
      if (stream.via) usePlayer.setState({ route: "matched" });
      this.graph.setDeck(this.cur.audio, 1);
      this.attach(this.cur, stream, () => token === this.loadToken && usePlayer.setState({ status: "error", error: "Поток прервался" }));
      this.applyEffects();
      await this.cur.audio.play();
      this.applyResume();
    } catch (error) {
      if (token !== this.loadToken || controller.signal.aborted) return;
      usePlayer.setState({ status: "error", error: error instanceof Error ? error.message : String(error) });
    }
  }

  // ---- the next song on the other deck -----------------------------------------------

  private nearEnd(audio: HTMLAudioElement): void {
    const duration = audio.duration;
    if (!Number.isFinite(duration) || duration <= 0) return;
    const left = (duration - audio.currentTime) * 1000;
    const fade = useSettings.getState().crossfade;
    if (left < PREPARE_BEFORE_MS + fade * 1000 && !this.prepared) void this.prepareNext();
    if (fade > 0 && !this.transitioning && this.prepared?.ready && left <= fade * 1000 && usePlayer.getState().repeat !== "one") {
      this.transition(Math.min(fade, left / 1000));
    }
  }

  private async prepareNext(): Promise<void> {
    const state = usePlayer.getState();
    if (state.repeat === "one" || this.mode !== "audio") return;
    const index = nextIndex(state, false);
    const track = state.queue[index];
    if (index < 0 || !track || index === state.index) return;
    const prepared: Prepared = { index, track, ready: false };
    this.prepared = prepared;
    this.prepareAbort?.abort();
    const controller = new AbortController();
    this.prepareAbort = controller;
    try {
      const stream = await resolveStream(track, controller.signal);
      if (this.prepared !== prepared) return;
      // the official YouTube player cannot be cued on a deck: that song loads when its turn comes
      if (stream.kind === "embed") {
        this.prepared = null;
        return;
      }
      if (stream.via) prepared.route = "matched";
      const deck = this.spare;
      this.unload(deck);
      this.graph.setDeck(deck.audio, 0);
      this.attach(deck, stream, () => this.prepared === prepared && this.dropPrepared());
    } catch {
      if (this.prepared === prepared) this.prepared = null;
    }
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
      const token = this.loadToken;
      setTimeout(() => {
        this.transitioning = false;
        if (token === this.loadToken || this.cur === now) this.unload(old);
      }, fade * 1000 + 150);
    } else {
      this.graph.setDeck(now.audio, 1);
      this.unload(old);
    }
    usePlayer.setState({ index: prepared.index, status: "playing", positionMs: 0, durationMs: prepared.track.durationMs, error: undefined, route: prepared.route });
    this.begin(prepared.track);
    void now.audio.play().catch(() => usePlayer.setState({ status: "paused" }));
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
      artwork: track.artwork && !track.artwork.startsWith("evplayer-media:") ? [{ src: track.artwork, sizes: "500x500" }] : [],
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
