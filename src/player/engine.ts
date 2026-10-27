import Hls from "hls.js";
import type { Track } from "@/types";
import { currentTrack, nextIndex, prevIndex, shuffledQueue, usePlayer } from "./store";
import { resolveStream } from "./streams";
import { createAudioGraph, type AudioGraph } from "@/audio/graph";
import { effectsForNextTrack, useEffects, type EffectsState } from "./effects";

// One <audio> element for the whole app. HLS streams go through hls.js, plain files play
// directly. The OS media controls (keyboard keys, the Windows volume flyout) are wired via
// the Media Session API.

class PlayerEngine {
  readonly audio: HTMLAudioElement;
  private hls: Hls | null = null;
  private loadAbort: AbortController | null = null;
  private loadToken = 0;
  private graph: AudioGraph;
  private firstTrack = true;

  constructor() {
    this.audio = new Audio();
    this.audio.crossOrigin = "anonymous";
    this.audio.preload = "auto";
    const state = usePlayer.getState();
    this.audio.volume = state.volume;

    this.audio.addEventListener("timeupdate", () => usePlayer.setState({ positionMs: this.audio.currentTime * 1000 }));
    this.audio.addEventListener("durationchange", () => {
      const d = this.audio.duration;
      if (Number.isFinite(d)) usePlayer.setState({ durationMs: d * 1000 });
    });
    this.audio.addEventListener("playing", () => usePlayer.setState({ status: "playing", error: undefined }));
    this.audio.addEventListener("pause", () => {
      if (usePlayer.getState().status === "playing") usePlayer.setState({ status: "paused" });
    });
    this.audio.addEventListener("ended", () => this.advance(false));
    this.audio.addEventListener("error", () => {
      if (usePlayer.getState().status !== "loading") usePlayer.setState({ status: "error", error: "Не удалось воспроизвести трек" });
    });

    usePlayer.subscribe((s, prev) => {
      if (s.volume !== prev.volume || s.muted !== prev.muted) {
        this.audio.volume = s.volume;
        this.audio.muted = s.muted;
      }
    });
    this.setupMediaSession();

    this.graph = createAudioGraph(this.audio);
    useEffects.subscribe((fx) => this.applyEffects(fx));
    this.audio.addEventListener("play", () => this.graph.resume());
  }

  private applyEffects(fx: EffectsState = useEffects.getState()): void {
    this.audio.defaultPlaybackRate = fx.speed;
    this.audio.playbackRate = fx.speed;
    // linked: the speed also moves the pitch, the way a faster record sounds higher
    this.audio.preservesPitch = !fx.linked;
    void this.graph
      .apply({ pitchSemitones: fx.linked ? 0 : fx.pitch, reverb: fx.reverb, reverbAmount: fx.reverbAmount })
      .catch((err) => console.warn("[effects]", err));
  }

  async playQueue(tracks: Track[], startIndex = 0): Promise<void> {
    if (tracks.length === 0) return;
    const { shuffle } = usePlayer.getState();
    const start = tracks[startIndex] ?? tracks[0];
    const queue = shuffle ? shuffledQueue(tracks, start.id) : tracks;
    const index = shuffle ? 0 : Math.max(0, startIndex);
    usePlayer.setState({ queue, index });
    await this.load(queue[index]);
  }

  async playIndex(index: number): Promise<void> {
    const track = usePlayer.getState().queue[index];
    if (!track) return;
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

  toggle(): void {
    const { status } = usePlayer.getState();
    if (status === "playing") this.audio.pause();
    else if (status === "paused") void this.audio.play().catch(() => {});
    else if (status === "error" || status === "idle") {
      const track = currentTrack();
      if (track) void this.load(track);
    }
  }

  next(): void {
    this.advance(true);
  }

  prev(): void {
    // like most players: a few seconds in, "previous" restarts the song
    if (this.audio.currentTime > 4) {
      this.seek(0);
      return;
    }
    const i = prevIndex(usePlayer.getState());
    if (i >= 0) void this.playIndex(i);
  }

  seek(ms: number): void {
    if (!Number.isFinite(ms)) return;
    this.audio.currentTime = Math.max(0, ms / 1000);
    usePlayer.setState({ positionMs: ms });
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
      usePlayer.setState({ status: "paused" });
      return;
    }
    if (i === state.index && !manual) {
      this.seek(0);
      void this.audio.play().catch(() => {});
      return;
    }
    void this.playIndex(i);
  }

  private async load(track: Track): Promise<void> {
    const token = ++this.loadToken;
    this.loadAbort?.abort();
    const controller = new AbortController();
    this.loadAbort = controller;
    this.detachHls();
    this.audio.pause();
    usePlayer.setState({ status: "loading", positionMs: 0, durationMs: track.durationMs, error: undefined });
    this.updateMetadata(track);
    if (!this.firstTrack) useEffects.setState(effectsForNextTrack(useEffects.getState()));
    this.firstTrack = false;
    try {
      const stream = await resolveStream(track, controller.signal);
      if (token !== this.loadToken) return;
      if (stream.hls && Hls.isSupported()) {
        const hls = new Hls({ enableWorker: true, maxBufferLength: 30 });
        this.hls = hls;
        hls.loadSource(stream.url);
        hls.attachMedia(this.audio);
        hls.on(Hls.Events.ERROR, (_event, data) => {
          if (data.fatal && token === this.loadToken) usePlayer.setState({ status: "error", error: "Поток прервался" });
        });
      } else {
        this.audio.src = stream.url;
      }
      this.applyEffects();
      await this.audio.play();
    } catch (error) {
      if (token !== this.loadToken || controller.signal.aborted) return;
      usePlayer.setState({ status: "error", error: error instanceof Error ? error.message : String(error) });
    }
  }

  private detachHls(): void {
    if (this.hls) {
      this.hls.destroy();
      this.hls = null;
    }
    this.audio.removeAttribute("src");
    this.audio.load();
  }

  private updateMetadata(track: Track): void {
    if (!("mediaSession" in navigator)) return;
    navigator.mediaSession.metadata = new MediaMetadata({
      title: track.title,
      artist: track.artists.map((a) => a.name).join(", "),
      artwork: track.artwork ? [{ src: track.artwork, sizes: "500x500" }] : [],
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
