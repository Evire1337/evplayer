import { Menu, Tray, nativeImage, type BrowserWindow } from "electron";

// Keeps EvPlayer in the notification area: closing (or, if chosen, minimizing) the window
// hides it there while music keeps playing; the tray icon brings it back and has the
// basic playback controls.

export interface TrayOptions {
  closeToTray: boolean;
  minimizeToTray: boolean;
  /** language chosen in the app (the system language may differ) */
  locale?: string;
}

export type TrayAction = "prev" | "togglePlay" | "next" | "toggleOverlay";

interface TrayTrack {
  title?: string;
  artist?: string;
}

const PLAYING = new Set(["playing", "loading", "buffering"]);

function strings(locale: string) {
  const ru = /^(ru|uk|be)/i.test(locale);
  return ru
    ? { show: "Показать EvPlayer", hide: "Скрыть в трей", play: "Играть", pause: "Пауза", overlay: "Мини-плеер", next: "Следующий", prev: "Предыдущий", quit: "Выход", nothing: "Ничего не играет", balloonTitle: "EvPlayer работает в трее", balloon: "Музыка продолжает играть. Нажмите на значок, чтобы открыть окно." }
    : { show: "Show EvPlayer", hide: "Hide to tray", play: "Play", pause: "Pause", overlay: "Mini player", next: "Next", prev: "Previous", quit: "Quit", nothing: "Nothing playing", balloonTitle: "EvPlayer is in the tray", balloon: "Music keeps playing. Click the icon to open the window." };
}

export class TrayController {
  private tray: Tray | null = null;
  private options: TrayOptions = { closeToTray: true, minimizeToTray: false };
  private track: TrayTrack | null = null;
  private playing = false;
  private balloonShown = false;
  private text: ReturnType<typeof strings>;

  constructor(
    private readonly iconPath: string,
    private readonly getWindow: () => BrowserWindow | null,
    private readonly send: (action: TrayAction) => void,
    private readonly quit: () => void,
    locale: string,
  ) {
    this.text = strings(locale);
  }

  create(): void {
    if (this.tray) return;
    const image = nativeImage.createFromPath(this.iconPath).resize({ width: 32, height: 32, quality: "best" });
    this.tray = new Tray(image);
    this.tray.setToolTip("EvPlayer");
    this.tray.on("click", () => this.toggleWindow());
    this.tray.on("double-click", () => this.showWindow());
    this.rebuild();
  }

  destroy(): void {
    this.tray?.destroy();
    this.tray = null;
  }

  getOptions(): TrayOptions {
    return this.options;
  }

  setOptions(next: Partial<TrayOptions>): void {
    const { locale, ...rest } = next;
    this.options = { ...this.options, ...rest };
    if (typeof locale === "string" && locale) {
      this.text = strings(locale);
      this.rebuild();
    }
  }

  update(state: { status?: string; track?: TrayTrack | null } | null | undefined): void {
    const playing = PLAYING.has(state?.status ?? "");
    const track = state?.track ?? null;
    if (playing === this.playing && track?.title === this.track?.title && track?.artist === this.track?.artist) return;
    this.playing = playing;
    this.track = track;
    this.rebuild();
  }

  /** hide the window into the tray; tells the user once per session where it went */
  hideWindow(): void {
    const win = this.getWindow();
    if (!win || win.isDestroyed()) return;
    win.hide();
    this.rebuild();
    if (!this.balloonShown && this.tray && process.platform === "win32") {
      this.balloonShown = true;
      try {
        this.tray.displayBalloon({ title: this.text.balloonTitle, content: this.text.balloon, iconType: "none" });
      } catch {
        // balloons are cosmetic
      }
    }
  }

  showWindow(): void {
    const win = this.getWindow();
    if (!win || win.isDestroyed()) return;
    if (win.isMinimized()) win.restore();
    win.show();
    win.focus();
    this.rebuild();
  }

  private toggleWindow(): void {
    const win = this.getWindow();
    if (!win || win.isDestroyed()) return;
    if (win.isVisible() && !win.isMinimized()) this.hideWindow();
    else this.showWindow();
  }

  private rebuild(): void {
    if (!this.tray) return;
    const t = this.text;
    const win = this.getWindow();
    const visible = Boolean(win && !win.isDestroyed() && win.isVisible() && !win.isMinimized());
    const label = this.track?.title ? `${this.track.title}${this.track.artist ? ` — ${this.track.artist}` : ""}` : t.nothing;
    this.tray.setToolTip(this.track?.title ? `EvPlayer · ${label}`.slice(0, 127) : "EvPlayer");
    this.tray.setContextMenu(
      Menu.buildFromTemplate([
        { label: label.length > 60 ? `${label.slice(0, 57)}…` : label, enabled: false },
        { type: "separator" },
        { label: this.playing ? t.pause : t.play, enabled: Boolean(this.track), click: () => this.send("togglePlay") },
        { label: t.next, enabled: Boolean(this.track), click: () => this.send("next") },
        { label: t.prev, enabled: Boolean(this.track), click: () => this.send("prev") },
        { label: t.overlay, click: () => this.send("toggleOverlay") },
        { type: "separator" },
        visible ? { label: t.hide, click: () => this.hideWindow() } : { label: t.show, click: () => this.showWindow() },
        { label: t.quit, click: () => this.quit() },
      ]),
    );
  }
}
