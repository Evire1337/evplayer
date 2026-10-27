import { BrowserWindow, screen } from "electron";

// The mini player: a small always-on-top window with the cover, the title and the basic
// buttons. It is a second page of the same app (index.html#overlay) that only shows what
// the main window tells it, and sends its button presses back.

export type OverlayCorner = "top-left" | "top-center" | "top-right" | "bottom-left" | "bottom-center" | "bottom-right";

export interface OverlayOptions {
  enabled: boolean;
  onTop: boolean;
  onMinimize: boolean;
  corner: OverlayCorner;
}

// a slim bar: small enough to sit over a game without covering anything important
export const OVERLAY_SIZE = { width: 330, height: 52 };
const MARGIN = 24;

export function cornerPosition(corner: OverlayCorner, area: { x: number; y: number; width: number; height: number }, size = OVERLAY_SIZE): { x: number; y: number } {
  const left = corner.endsWith("left");
  const center = corner.endsWith("center");
  const top = corner.startsWith("top");
  return {
    x: Math.round(center ? area.x + (area.width - size.width) / 2 : left ? area.x + MARGIN : area.x + area.width - size.width - MARGIN),
    y: Math.round(top ? area.y + MARGIN : area.y + area.height - size.height - MARGIN),
  };
}

export class OverlayController {
  private win: BrowserWindow | null = null;
  private options: OverlayOptions = { enabled: true, onTop: true, onMinimize: false, corner: "bottom-right" };
  private lastState: unknown = null;
  /** where the user dragged it; null = use the corner */
  private moved: { x: number; y: number } | null = null;
  private hideTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly load: (win: BrowserWindow) => void,
    private readonly preload: string,
    private readonly icon: string,
    private readonly onVisible: (visible: boolean) => void,
  ) {}

  getOptions(): OverlayOptions {
    return this.options;
  }

  setOptions(next: Partial<OverlayOptions>): void {
    const cornerChanged = next.corner && next.corner !== this.options.corner;
    this.options = { ...this.options, ...next };
    if (!this.options.enabled) this.hide();
    if (this.win && !this.win.isDestroyed()) {
      this.win.setAlwaysOnTop(this.options.onTop, "floating");
      if (cornerChanged) {
        this.moved = null;
        this.place();
      }
    }
  }

  isVisible(): boolean {
    return Boolean(this.win && !this.win.isDestroyed() && this.win.isVisible());
  }

  toggle(): void {
    if (this.isVisible()) this.hide();
    else this.show();
  }

  show(): void {
    if (!this.options.enabled) return;
    if (this.hideTimer) clearTimeout(this.hideTimer);
    this.hideTimer = null;
    if (!this.win || this.win.isDestroyed()) this.create();
    this.place();
    this.win!.showInactive();
    this.onVisible(true);
  }

  hide(): void {
    if (!this.win || this.win.isDestroyed()) return;
    this.win.hide();
    this.onVisible(false);
    // a hidden mini player is closed after a while so it costs no memory
    if (this.hideTimer) clearTimeout(this.hideTimer);
    this.hideTimer = setTimeout(() => this.destroy(), 60_000);
  }

  destroy(): void {
    if (this.hideTimer) clearTimeout(this.hideTimer);
    this.hideTimer = null;
    if (this.win && !this.win.isDestroyed()) this.win.destroy();
    this.win = null;
  }

  /** player state from the main window */
  send(state: unknown): void {
    this.lastState = state;
    if (this.win && !this.win.isDestroyed()) this.win.webContents.send("overlay:state", state);
  }

  resendState(): void {
    if (this.lastState) this.send(this.lastState);
  }

  private place(): void {
    if (!this.win) return;
    if (this.moved) {
      this.win.setPosition(this.moved.x, this.moved.y);
      return;
    }
    const area = screen.getPrimaryDisplay().workArea;
    const { x, y } = cornerPosition(this.options.corner, area);
    this.win.setPosition(x, y);
  }

  private create(): void {
    const win = new BrowserWindow({
      ...OVERLAY_SIZE,
      show: false,
      frame: false,
      transparent: true,
      resizable: false,
      maximizable: false,
      minimizable: false,
      fullscreenable: false,
      skipTaskbar: true,
      alwaysOnTop: this.options.onTop,
      hasShadow: false,
      backgroundColor: "#00000000",
      title: "EvPlayer — мини-плеер",
      icon: this.icon,
      webPreferences: { preload: this.preload, contextIsolation: true, nodeIntegration: false, sandbox: true, backgroundThrottling: true },
    });
    win.setAlwaysOnTop(this.options.onTop, "floating");
    win.on("moved", () => {
      const [x, y] = win.getPosition();
      this.moved = { x, y };
    });
    win.on("closed", () => {
      if (this.win === win) this.win = null;
      this.onVisible(false);
    });
    this.win = win;
    this.load(win);
  }
}
