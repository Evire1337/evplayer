import { app, BrowserWindow, dialog, globalShortcut, ipcMain, protocol, session, shell } from "electron";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isOpenHost, openCorsHeaders } from "./cors.js";
import { getSoundCloudClientId } from "./soundcloudClientId.js";
import { setupUpdates } from "./updates.js";
import { LOCAL_SCHEME, LocalLibrary, type LocalTrackEdit } from "./local-files.js";
import { TrayController, type TrayAction, type TrayOptions } from "./tray.js";
import { DiscordPresence, type PresenceState } from "./discord.js";
import { refreshSpotify, signInWithSpotify } from "./spotifyAuth.js";
import { OverlayController, type OverlayOptions } from "./overlay.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DIST = path.join(__dirname, "../dist");
const PUBLIC = app.isPackaged ? DIST : path.join(__dirname, "../public");
const DEV_URL = process.env.VITE_DEV_SERVER_URL;
const APP_REFERER = "https://github.com/Evire1337/evplayer";

let mainWindow: BrowserWindow | null = null;
let quitting = false;

// the scheme for the user's own files must be registered before the app is ready
protocol.registerSchemesAsPrivileged([
  { scheme: LOCAL_SCHEME, privileges: { standard: true, secure: true, supportFetchAPI: true, stream: true, corsEnabled: true } },
]);

const localLibrary = new LocalLibrary(app.getPath("userData"));
const discord = new DiscordPresence();
const tray = new TrayController(
  path.join(PUBLIC, "icon.png"),
  () => mainWindow,
  (action: TrayAction) => mainWindow?.webContents.send("player:command", action),
  () => {
    quitting = true;
    app.quit();
  },
  app.getLocale(),
);

function fromYouTubeFrame(referrer: string | undefined, frameUrl: string | undefined): boolean {
  return [referrer, frameUrl].some((u) => Boolean(u) && /^https:\/\/(www\.)?youtube(-nocookie)?\.com\//.test(u!));
}

function loadPage(win: BrowserWindow, hash = ""): void {
  if (DEV_URL) void win.loadURL(`${DEV_URL}${hash ? `#${hash}` : ""}`);
  else void win.loadFile(path.join(DIST, "index.html"), hash ? { hash } : undefined);
}

const overlay = new OverlayController(
  (win) => loadPage(win, "overlay"),
  path.join(__dirname, "preload.cjs"),
  path.join(PUBLIC, "icon.png"),
  (visible) => mainWindow?.webContents.send("overlay:visible", visible),
);

function createWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 900,
    minHeight: 600,
    show: false,
    backgroundColor: "#06201d",
    title: "EvPlayer",
    icon: path.join(PUBLIC, "icon.png"),
    titleBarStyle: "hidden",
    titleBarOverlay: { color: "#00000000", symbolColor: "#eef7f4", height: 40 },
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      // music has to keep playing and counting while the window is minimized
      backgroundThrottling: false,
    },
  });
  mainWindow.once("ready-to-show", () => mainWindow?.show());
  mainWindow.on("closed", () => (mainWindow = null));
  mainWindow.on("close", (event) => {
    if (quitting || !tray.getOptions().closeToTray) return;
    event.preventDefault();
    tray.hideWindow();
  });
  mainWindow.on("minimize", () => {
    if (tray.getOptions().minimizeToTray) tray.hideWindow();
    if (overlay.getOptions().onMinimize) overlay.show();
  });
  mainWindow.on("restore", () => {
    if (overlay.getOptions().onMinimize) overlay.hide();
  });
  // the page pauses its animations while it cannot be seen
  const visible = (value: boolean) => () => mainWindow?.webContents.send("window:visible", value);
  mainWindow.on("hide", visible(false));
  mainWindow.on("minimize", visible(false));
  mainWindow.on("show", visible(true));
  mainWindow.on("restore", visible(true));
  // F12 or Ctrl+Shift+I: developer tools, for looking into problems
  mainWindow.webContents.on("before-input-event", (event, input) => {
    if (input.type !== "keyDown") return;
    const devtools = input.key === "F12" || (input.control && input.shift && input.key.toLowerCase() === "i");
    if (devtools) {
      event.preventDefault();
      mainWindow?.webContents.toggleDevTools();
    }
  });
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    if (/^https?:/i.test(url)) void shell.openExternal(url);
    return { action: "deny" };
  });
  loadPage(mainWindow);
}

function setupLocalFiles(): void {
  protocol.handle(LOCAL_SCHEME, (request) => localLibrary.handle(request));
  const changed = () => mainWindow?.webContents.send("local:changed");
  void localLibrary.startWatching(changed);

  ipcMain.handle("local:list", () => localLibrary.list());
  ipcMain.handle("local:pick", async (_e, kind: "files" | "folder") => {
    const win = mainWindow;
    const options: Electron.OpenDialogOptions = kind === "folder"
      ? { properties: ["openDirectory"] }
      : {
          properties: ["openFile", "multiSelections"],
          filters: [{ name: "Audio", extensions: ["mp3", "flac", "m4a", "mp4", "aac", "ogg", "oga", "opus", "wav", "webm"] }],
        };
    const result = win ? await dialog.showOpenDialog(win, options) : await dialog.showOpenDialog(options);
    if (result.canceled || result.filePaths.length === 0) return { added: [], skipped: 0 };
    return localLibrary.add(result.filePaths);
  });
  ipcMain.handle("local:add-paths", (_e, paths: string[]) => localLibrary.add(paths.filter((p) => typeof p === "string")));
  ipcMain.handle("local:remove", (_e, ids: string[]) => localLibrary.remove(ids));
  ipcMain.handle("local:update", (_e, id: string, patch: LocalTrackEdit) => localLibrary.update(id, patch));
  ipcMain.handle("local:reveal", async (_e, id: string) => {
    const record = await localLibrary.get(id);
    if (record) shell.showItemInFolder(record.path);
  });
  ipcMain.handle("local:watched-folders", () => localLibrary.watchedFolders());
  ipcMain.handle("local:unwatch-folder", (_e, folder: string) => localLibrary.unwatchFolder(folder));
}

function setupFileDialogs(): void {
  // backups and exports: the page hands over text, the user picks where it goes
  ipcMain.handle("file:save-text", async (_e, name: string, text: string) => {
    const options = { defaultPath: path.join(app.getPath("documents"), name), filters: [{ name: "JSON", extensions: ["json"] }] };
    const result = mainWindow ? await dialog.showSaveDialog(mainWindow, options) : await dialog.showSaveDialog(options);
    if (result.canceled || !result.filePath) return false;
    await fs.writeFile(result.filePath, text, "utf8");
    return true;
  });
  ipcMain.handle("file:open-text", async (_e, extensions: string[]) => {
    const options: Electron.OpenDialogOptions = { properties: ["openFile", "multiSelections"], filters: [{ name: "Data", extensions }] };
    const result = mainWindow ? await dialog.showOpenDialog(mainWindow, options) : await dialog.showOpenDialog(options);
    if (result.canceled) return [];
    return Promise.all(result.filePaths.map(async (file) => ({ name: path.basename(file), text: await fs.readFile(file, "utf8") })));
  });
}

/** system-wide keys chosen in the settings: accelerator -> command for the page */
function setGlobalShortcuts(map: Record<string, string>): string[] {
  globalShortcut.unregisterAll();
  const failed: string[] = [];
  for (const [accelerator, action] of Object.entries(map ?? {})) {
    try {
      const ok = globalShortcut.register(accelerator, () => {
        if (action === "showWindow") tray.showWindow();
        else if (action === "toggleOverlay") overlay.toggle();
        else mainWindow?.webContents.send("player:command", action);
      });
      if (!ok) failed.push(accelerator);
    } catch {
      failed.push(accelerator);
    }
  }
  return failed;
}

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on("second-instance", () => tray.showWindow());

  app.whenReady().then(() => {
    const filter = { urls: ["https://*/*"] };
    session.defaultSession.webRequest.onHeadersReceived(filter, (details, callback) => {
      // the YouTube player inside its own frame handles CORS itself: leave its traffic alone
      if (!isOpenHost(details.url) || fromYouTubeFrame(details.referrer, details.frame?.url)) return callback({});
      const preflight = details.method === "OPTIONS";
      callback({
        responseHeaders: openCorsHeaders(details.responseHeaders),
        ...(preflight ? { statusLine: "HTTP/1.1 200 OK" } : {}),
      });
    });

    // The YouTube player refuses to start without knowing which site embeds it. A desktop
    // app has no site, so it names its project page — the same thing a web page would send.
    const embedFilter = { urls: ["https://www.youtube.com/embed/*", "https://www.youtube-nocookie.com/embed/*"] };
    session.defaultSession.webRequest.onBeforeSendHeaders(embedFilter, (details, callback) => {
      callback({ requestHeaders: { ...details.requestHeaders, Referer: APP_REFERER } });
    });

    ipcMain.handle("app:version", () => app.getVersion());
    ipcMain.handle("spotify:sign-in", (_e, clientId: string) => signInWithSpotify(String(clientId)));
    ipcMain.handle("spotify:refresh", (_e, clientId: string, refreshToken: string) => refreshSpotify(String(clientId), String(refreshToken)));
    ipcMain.handle("app:data-size", async () => {
      const info = await session.defaultSession.getCacheSize();
      return { cache: info };
    });
    ipcMain.handle("app:clear-cache", () => session.defaultSession.clearCache());
    ipcMain.handle("app:open-data-folder", () => shell.openPath(app.getPath("userData")));
    ipcMain.handle("soundcloud:client-id", (_event, force: boolean) => getSoundCloudClientId(Boolean(force)));

    ipcMain.on("tray:set-options", (_e, options: Partial<TrayOptions>) => tray.setOptions(options));
    ipcMain.on("tray:update", (_e, state: { status?: string; track?: { title?: string; artist?: string } | null }) => tray.update(state));
    ipcMain.handle("shortcuts:global", (_e, map: Record<string, string>) => setGlobalShortcuts(map));
    ipcMain.on("window:show", () => tray.showWindow());
    ipcMain.on("overlay:set-options", (_e, options: Partial<OverlayOptions>) => overlay.setOptions(options));
    ipcMain.on("overlay:toggle", () => overlay.toggle());
    ipcMain.on("overlay:hide", () => overlay.hide());
    ipcMain.on("overlay:state", (_e, state: unknown) => overlay.send(state));
    ipcMain.on("overlay:ready", () => {
      overlay.resendState();
      mainWindow?.webContents.send("overlay:ready");
    });
    ipcMain.on("overlay:command", (_e, action: string, value?: number) => {
      if (action === "showWindow") tray.showWindow();
      else mainWindow?.webContents.send("player:command", action, value);
    });
    ipcMain.on("discord:set", (_e, appId: string | null) => discord.setAppId(typeof appId === "string" ? appId : null));
    ipcMain.on("discord:update", (_e, state: PresenceState | null) => discord.update(state));
    ipcMain.handle("discord:status", () => discord.status());

    setupLocalFiles();
    setupFileDialogs();
    createWindow();
    tray.create();
    setupUpdates(() => mainWindow);
  });

  app.on("before-quit", () => {
    quitting = true;
    localLibrary.stopWatching();
    discord.destroy();
    overlay.destroy();
    tray.destroy();
  });
  app.on("will-quit", () => globalShortcut.unregisterAll());
  app.on("window-all-closed", () => {
    if (process.platform !== "darwin") app.quit();
  });
}
