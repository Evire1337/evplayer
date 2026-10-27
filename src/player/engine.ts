import Hls from "hls.js";
import type { Track } from "@/types";
import { currentTrack, nextIndex, prevIndex, shuffledQueue, usePlayer } from "./store";
import { resolveStream } from "./streams";
import { createAudioGraph, type AudioGraph } from "@/audio/graph";
import { setLevelingTrack } from "@/equalizer/eqAudio";
import { effectsForNextTrack, useEffects, type EffectsState } from "./effects";
import { YouTubeEmbed } from "./youtubeEmbed";
import { similarTracks } from "./radio";
import { useSettings } from "@/settings/store";
import { HISTORY_AFTER_MS, useHistory } from "@/stats/history";
import { recordListen } from "@/stats/listeningStats";

// One <audio> element for the whole app. HLS streams go through hls.js, plain files play
// directly, YouTube songs without a SoundCloud copy use the official YouTube player. The OS
// media controls (keyboard keys, the Windows volume flyout) are wired via the Media
// Session API.

type Mode = "audio" | "embed";

class PlayerEngine {
  readonly audio: HTMLAudioElement;
  private hls: Hls | null = null;
  private loadAbort: AbortController | null = null;
  private loadToken = 0;
  private graph: AudioGraph;
  private firstTrack = true;
  private mode: Mode = "audio";
  private embed: YouTubeEmbed;
  // listening: how much of the current song was actually heard
  private session = "";
  private sessionTrack: Track | null = null;
  private heardMs = 0;
  private lastPos = -1;
  private counted = false;
  private radioBusy = false;

  constructor() {
    this.audio = new Audio();
    this.audio.crossOrigin = "anonymous";
    this.audio.preload = "auto";
    const state = usePlayer.getState();
    this.audio.volume = state.volume;

    this.audio.addEventListener("timeupdate", () => {
      if (this.mode !== "audio") return;
      const pos = this.audio.currentTime * 1000;
      usePlayer.setState({ positionMs: pos });
      this.heard(pos, !this.audio.paused);
    });
    this.audio.addEventListener("durationchange", () => {
      const d = this.audio.duration;
      if (this.mode === "audio" && Number.isFinite(d)) usePlayer.setState({ durationMs: d * 1000 });
    });
    this.audio.addEventListener("playing", () => this.mode === "audio" && usePlayer.setState({ status: "playing", error: undefined }));
    this.audio.addEventListener("pause", () => {
      if (this.mode === "audio" && usePlayer.getState().status === "playing") usePlayer.setState({ status: "paused" });
    });
    this.audio.addEventListener("ended", () => this.mode === "audio" && this.advance(false));
    this.audio.addEventListener("error", () => {
      if (this.mode !== "audio" || !this.audio.getAttribute("src")) return;
      if (usePlayer.getState().status !== "loading") usePlayer.setState({ status: "error", error: "Не удалось воспроизвести трек" });
    });

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
        this.audio.volume = s.volume;
        this.audio.muted = s.muted;
        this.embed.setVolume(s.volume, s.muted);
      }
    });
    this.setupMediaSession();

    this.graph = createAudioGraph(this.audio);
    useEffects.subscribe((fx) => this.applyEffects(fx));
    this.audio.addEventListener("play", () => this.graph.resume());
    window.addEventListener("beforeunload", () => this.finishListen());
  }

  private applyEffects(fx: EffectsState = useEffects.getState()): void {
    this.audio.defaultPlaybackRate = fx.speed;
    this.audio.playbackRate = fx.speed;
    // linked: the speed also moves the pitch, the way a faster record sounds higher
    this.audio.preservesPitch = !fx.linked;
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
    else this.audio.pause();
  }

  private resume(): void {
    if (this.mode === "embed") this.embed.play();
    else void this.audio.play().catch(() => {});
  }

  next(): void {
    this.advance(true);
  }

  prev(): void {
    // like most players: a few seconds in, "previous" restarts the song
    if (usePlayer.getState().positionMs > 4000) {
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
    else this.audio.currentTime = target / 1000;
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
      if (useSettings.getState().autoplay) void this.extendWithSimilar();
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

  private async extendWithSimilar(): Promise<void> {
    const state = usePlayer.getState();
    const seed = currentTrack(state);
    if (!seed || this.radioBusy) return;
    this.radioBusy = true;
    try {
      const more = await similarTracks(seed, new Set(state.queue.map((t) => t.id)));
      if (!more.length) {
        usePlayer.setState({ status: "paused" });
        return;
      }
      const queue = [...usePlayer.getState().queue, ...more];
      usePlayer.setState({ queue });
      await this.playIndex(state.index + 1);
    } catch {
      usePlayer.setState({ status: "paused" });
    } finally {
      this.radioBusy = false;
    }
  }

  private async load(track: Track): Promise<void> {
    const token = ++this.loadToken;
    this.loadAbort?.abort();
    const controller = new AbortController();
    this.loadAbort = controller;
    this.detachHls();
    this.audio.pause();
    this.startListen(track);
    usePlayer.setState({ status: "loading", positionMs: 0, durationMs: track.durationMs, error: undefined, route: undefined });
    this.updateMetadata(track);
    if (!this.firstTrack) useEffects.setState(effectsForNextTrack(useEffects.getState()));
    setLevelingTrack(track.id);
    this.firstTrack = false;
    try {
      const stream = await resolveStream(track, controller.signal);
      if (token !== this.loadToken) return;
      if (stream.kind === "embed") {
        this.mode = "embed";
        usePlayer.setState({ route: "embed" });
        const { volume, muted } = usePlayer.getState();
        await this.embed.load(stream.videoId, volume, muted);
        return;
      }
      this.mode = "audio";
      this.embed.stop();
      if (stream.via) usePlayer.setState({ route: "matched" });
      if (stream.hls && Hls.isSupported()) {
        const hls = new Hls({ enableWorker: true, maxBufferLength: 30, backBufferLength: 30 });
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
    if (this.audio.getAttribute("src")) {
      this.audio.removeAttribute("src");
      this.audio.load();
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
