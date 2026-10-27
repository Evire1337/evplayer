// The official YouTube player, for YouTube songs that have no SoundCloud copy. YouTube's
// rules say the player must stay visible (at least 200×200) and keep its own controls and
// ads, so it sits in a small card in the corner while it plays. Our equalizer and effects
// cannot reach the sound inside it.

type YTPlayer = {
  loadVideoById(options: { videoId: string; startSeconds?: number }): void;
  cueVideoById(options: { videoId: string; startSeconds?: number }): void;
  playVideo(): void;
  pauseVideo(): void;
  stopVideo(): void;
  seekTo(seconds: number, allowSeekAhead: boolean): void;
  setVolume(volume: number): void;
  mute(): void;
  unMute(): void;
  setPlaybackRate(rate: number): void;
  getCurrentTime(): number;
  getDuration(): number;
  getPlayerState(): number;
  destroy(): void;
};

declare global {
  interface Window {
    YT?: { Player: new (el: HTMLElement, options: unknown) => YTPlayer; PlayerState: Record<string, number> };
    onYouTubeIframeAPIReady?: () => void;
  }
}

export interface EmbedEvents {
  onPlaying(): void;
  onPaused(): void;
  onEnded(): void;
  onBuffering(): void;
  onTime(positionMs: number, durationMs: number): void;
  onError(message: string): void;
}

let apiPromise: Promise<void> | null = null;

function loadApi(): Promise<void> {
  apiPromise ??= new Promise<void>((resolve, reject) => {
    if (window.YT?.Player) return resolve();
    const previous = window.onYouTubeIframeAPIReady;
    window.onYouTubeIframeAPIReady = () => {
      previous?.();
      resolve();
    };
    const script = document.createElement("script");
    script.src = "https://www.youtube.com/iframe_api";
    script.async = true;
    script.onerror = () => {
      apiPromise = null;
      reject(new Error("YouTube player did not load"));
    };
    document.head.appendChild(script);
  });
  return apiPromise;
}

function errorText(code: number): string {
  if (code === 101 || code === 150) return "Автор запретил воспроизведение этого видео вне YouTube";
  if (code === 100) return "Видео удалено или скрыто";
  if (code === 152 || code === 153) return "YouTube не пустил плеер (ошибка " + code + ")";
  return "YouTube не смог воспроизвести видео (" + code + ")";
}

export class YouTubeEmbed {
  private player: YTPlayer | null = null;
  private host: HTMLDivElement | null = null;
  private poll: ReturnType<typeof setInterval> | null = null;
  private ready: Promise<YTPlayer> | null = null;
  active = false;

  constructor(private readonly events: EmbedEvents) {}

  private ensureHost(): HTMLDivElement {
    if (this.host) return this.host;
    const host = document.createElement("div");
    host.className = "yt-embed";
    host.setAttribute("aria-label", "YouTube-плеер");
    const label = document.createElement("div");
    label.className = "yt-embed-label";
    label.textContent = "Играет через YouTube";
    const slot = document.createElement("div");
    host.append(label, slot);
    document.body.appendChild(host);
    this.host = host;
    return host;
  }

  private create(): Promise<YTPlayer> {
    this.ready ??= loadApi().then(
      () =>
        new Promise<YTPlayer>((resolve) => {
          const host = this.ensureHost();
          const slot = host.lastElementChild as HTMLElement;
          const player: YTPlayer = new window.YT!.Player(slot, {
            width: "100%",
            height: "100%",
            host: "https://www.youtube-nocookie.com",
            playerVars: { autoplay: 1, playsinline: 1, rel: 0, modestbranding: 1 },
            events: {
              onReady: () => resolve(player),
              onStateChange: (e: { data: number }) => this.onState(e.data),
              onError: (e: { data: number }) => this.events.onError(errorText(e.data)),
            },
          });
          this.player = player;
        }),
    );
    return this.ready;
  }

  private onState(state: number): void {
    if (!this.active) return;
    // -1 unstarted, 0 ended, 1 playing, 2 paused, 3 buffering, 5 cued
    if (state === 1) this.events.onPlaying();
    else if (state === 2) this.events.onPaused();
    else if (state === 0) this.events.onEnded();
    else if (state === 3) this.events.onBuffering();
  }

  async load(videoId: string, volume: number, muted: boolean): Promise<void> {
    this.active = true;
    this.ensureHost().classList.add("is-visible");
    const player = await this.create();
    if (!this.active) return;
    player.setVolume(Math.round(volume * 100));
    if (muted) player.mute();
    else player.unMute();
    player.loadVideoById({ videoId });
    this.startPolling();
  }

  private startPolling(): void {
    if (this.poll) return;
    this.poll = setInterval(() => {
      if (!this.player || !this.active) return;
      const duration = this.player.getDuration() * 1000;
      this.events.onTime(this.player.getCurrentTime() * 1000, Number.isFinite(duration) ? duration : 0);
    }, 250);
  }

  play(): void {
    this.player?.playVideo();
  }

  pause(): void {
    this.player?.pauseVideo();
  }

  seek(ms: number): void {
    this.player?.seekTo(ms / 1000, true);
  }

  setVolume(volume: number, muted: boolean): void {
    if (!this.player) return;
    this.player.setVolume(Math.round(volume * 100));
    if (muted) this.player.mute();
    else this.player.unMute();
  }

  setRate(rate: number): void {
    this.player?.setPlaybackRate(rate);
  }

  /** hide the card and stop the video; the player itself is kept for the next YouTube song */
  stop(): void {
    this.active = false;
    if (this.poll) clearInterval(this.poll);
    this.poll = null;
    this.player?.stopVideo();
    this.host?.classList.remove("is-visible");
  }
}
