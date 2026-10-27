import { beforeEach, describe, expect, it, vi } from "vitest";

const menus: unknown[][] = [];
vi.mock("electron", () => {
  class Tray {
    handlers: Record<string, () => void> = {};
    setToolTip = vi.fn();
    setContextMenu = vi.fn();
    displayBalloon = vi.fn();
    destroy = vi.fn();
    on(event: string, handler: () => void) {
      this.handlers[event] = handler;
    }
  }
  return {
    Tray,
    Menu: { buildFromTemplate: (template: unknown[]) => (menus.push(template), template) },
    nativeImage: { createFromPath: () => ({ resize: () => ({}) }) },
  };
});

import { TrayController } from "./tray";

function fakeWindow() {
  let visible = true;
  return {
    isDestroyed: () => false,
    isVisible: () => visible,
    isMinimized: () => false,
    hide: vi.fn(() => (visible = false)),
    show: vi.fn(() => (visible = true)),
    restore: vi.fn(),
    focus: vi.fn(),
  };
}

describe("TrayController", () => {
  beforeEach(() => (menus.length = 0));

  it("closes to tray by default and can be switched off", () => {
    const tray = new TrayController("icon.png", () => null, vi.fn(), vi.fn(), "ru");
    expect(tray.getOptions()).toEqual({ closeToTray: true, minimizeToTray: false });
    tray.setOptions({ closeToTray: false, minimizeToTray: true });
    expect(tray.getOptions()).toEqual({ closeToTray: false, minimizeToTray: true });
    tray.setOptions({ locale: "en" });
    expect(tray.getOptions()).toEqual({ closeToTray: false, minimizeToTray: true });
  });

  it("hides and shows the window and sends playback actions", () => {
    const win = fakeWindow();
    const send = vi.fn();
    const quit = vi.fn();
    const tray = new TrayController("icon.png", () => win as never, send, quit, "ru");
    tray.create();
    tray.update({ status: "playing", track: { title: "Song", artist: "Artist" } });
    const menu = menus.at(-1) as Array<{ label?: string; click?: () => void }>;
    expect(menu[0].label).toBe("Song — Artist");
    menu.find((item) => item.label === "Пауза")?.click?.();
    expect(send).toHaveBeenCalledWith("togglePlay");
    tray.hideWindow();
    expect(win.hide).toHaveBeenCalled();
    tray.showWindow();
    expect(win.show).toHaveBeenCalled();
    (menus.at(-1) as Array<{ label?: string; click?: () => void }>).find((item) => item.label === "Выход")?.click?.();
    expect(quit).toHaveBeenCalled();
  });
});
