import { BrowserWindow, screen, type Rectangle } from "electron";

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
/** how often a visible mini player puts itself back on top (games keep pushing themselves up) */
const KEEP_ON_TOP_MS = 1500;
/** the highest level: above borderless full-screen games and the taskbar */
const TOP_LEVEL = "screen-saver" as const;

/** the dragged spot, if it is still on some screen; null when a screen went away or changed size */
export function onSomeScreen(point: { x: number; y: number }, areas: Rectangle[], size = OVERLAY_SIZE): boolean {
  return areas.some((a) => point.x >= a.x - size.width / 2 && point.y >= a.y && point.x + size.width / 2 <= a.x + a.width && point.y + size.height <= a.y + a.height);
}

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
  private options: OverlayOptions = { enabled: true, onTop: true, onMinimize: true, corner: "top-center" };
  private lastState: unknown = null;
  /** where the user dragged it; null = use the corner */
  private moved: { x: number; y: number } | null = null;
  private hideTimer: ReturnType<typeof setTimeout> | null = null;
  private keepTimer: ReturnType<typeof setInterval> | null = null;
  /** the user wants it on screen (shown by a key, the tray or minimizing) */
  private wanted = false;
  /** shown because the main window was minimized: restoring the window hides it again */
  private autoShown = false;
  private screensWatched = false;

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
      this.applyTop();
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

  /** the main window was minimized */
  showForMinimize(): void {
    if (this.isVisible()) return;
    this.show();
    this.autoShown = this.isVisible();
  }

  /** the main window came back: hide only a mini player that minimizing brought up */
  hideForRestore(): void {
    if (this.autoShown) this.hide();
  }

  show(): void {
    if (!this.options.enabled) return;
    if (this.hideTimer) clearTimeout(this.hideTimer);
    this.hideTimer = null;
    this.autoShown = false;
    this.wanted = true;
    this.watchScreens();
    if (!this.win || this.win.isDestroyed()) this.create();
    this.place();
    this.win!.showInactive();
    this.applyTop();
    this.startKeepOnTop();
    this.onVisible(true);
  }

  hide(): void {
    this.wanted = false;
    this.autoShown = false;
    this.stopKeepOnTop();
    if (!this.win || this.win.isDestroyed()) return;
    this.win.hide();
    this.onVisible(false);
    // a hidden mini player is closed after a while so it costs no memory
    if (this.hideTimer) clearTimeout(this.hideTimer);
    this.hideTimer = setTimeout(() => this.destroy(), 60_000);
  }

  destroy(): void {
    this.stopKeepOnTop();
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

  private applyTop(): void {
    const win = this.win;
    if (!win || win.isDestroyed()) return;
    if (this.options.onTop) {
      win.setAlwaysOnTop(true, TOP_LEVEL, 1);
      win.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
      win.moveTop();
    } else {
      win.setAlwaysOnTop(false);
    }
  }

  /**
   * Full-screen games put themselves on top too, and Windows lets the newest one win; it
   * also hides small tool windows on "show desktop" and when a game changes resolution.
   * While the mini player should be visible it checks itself every second and a half and
   * comes back: shown, not minimized, on a screen, above everything.
   */
  private startKeepOnTop(): void {
    if (this.keepTimer) return;
    this.keepTimer = setInterval(() => {
      const win = this.win;
      if (!this.wanted) return this.stopKeepOnTop();
      if (!win || win.isDestroyed()) {
        this.show();
        return;
      }
      if (win.isMinimized()) win.restore();
      if (!win.isVisible()) win.showInactive();
      if (this.options.onTop) {
        if (!win.isAlwaysOnTop()) win.setAlwaysOnTop(true, TOP_LEVEL, 1);
        win.moveTop();
      }
    }, KEEP_ON_TOP_MS);
  }

  private stopKeepOnTop(): void {
    if (this.keepTimer) clearInterval(this.keepTimer);
    this.keepTimer = null;
  }

  /** a game switching resolution or a monitor going away must not leave it off-screen */
  private watchScreens(): void {
    if (this.screensWatched) return;
    this.screensWatched = true;
    const replace = () => {
      if (this.win && !this.win.isDestroyed()) this.place();
    };
    screen.on("display-metrics-changed", replace);
    screen.on("display-added", replace);
    screen.on("display-removed", replace);
  }

  private place(): void {
    if (!this.win) return;
    const areas = screen.getAllDisplays().map((d) => d.workArea);
    if (this.moved && onSomeScreen(this.moved, areas)) {
      this.win.setBounds({ ...this.moved, ...OVERLAY_SIZE });
      return;
    }
    this.moved = null;
    const area = screen.getPrimaryDisplay().workArea;
    const { x, y } = cornerPosition(this.options.corner, area);
    // setBounds also restores the size: Windows sometimes stretches transparent windows on DPI changes
    this.win.setBounds({ x, y, ...OVERLAY_SIZE });
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
      webPreferences: { preload: this.preload, contextIsolation: true, nodeIntegration: false, sandbox: true, backgroundThrottling: false },
    });
    if (this.options.onTop) win.setAlwaysOnTop(true, TOP_LEVEL, 1);
    // never in the alt-tab list and never minimized together with other windows
    win.on("minimize", () => {
      if (this.wanted) win.restore();
    });
    // if its page crashes or hangs, start a fresh one in the same spot
    const recreate = () => {
      if (this.win !== win) return;
      this.win = null;
      if (!win.isDestroyed()) win.destroy();
      if (this.wanted) this.show();
    };
    win.webContents.on("render-process-gone", recreate);
    win.on("unresponsive", recreate);
    win.on("moved", () => {
      const [x, y] = win.getPosition();
      this.moved = { x, y };
    });
    win.on("closed", () => {
      if (this.win !== win) return;
      this.win = null;
      if (!this.wanted) this.onVisible(false);
    });
    this.win = win;
    this.load(win);
  }
}
