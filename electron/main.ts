import { app, BrowserWindow, ipcMain, session, shell } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isOpenHost, openCorsHeaders } from "./cors.js";
import { getSoundCloudClientId } from "./soundcloudClientId.js";
import { setupUpdates } from "./updates.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DIST = path.join(__dirname, "../dist");
const DEV_URL = process.env.VITE_DEV_SERVER_URL;

let mainWindow: BrowserWindow | null = null;

function createWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 900,
    minHeight: 600,
    show: false,
    backgroundColor: "#0d0c12",
    title: "EvPlayer",
    icon: path.join(app.isPackaged ? DIST : path.join(__dirname, "../public"), "icon.png"),
    titleBarStyle: "hidden",
    titleBarOverlay: { color: "#00000000", symbolColor: "#f2eff7", height: 40 },
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
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    if (/^https?:/i.test(url)) void shell.openExternal(url);
    return { action: "deny" };
  });
  if (DEV_URL) void mainWindow.loadURL(DEV_URL);
  else void mainWindow.loadFile(path.join(DIST, "index.html"));
}

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on("second-instance", () => {
    if (!mainWindow) return;
    if (mainWindow.isMinimized()) mainWindow.restore();
    mainWindow.show();
    mainWindow.focus();
  });

  app.whenReady().then(() => {
    const filter = { urls: ["https://*/*"] };
    session.defaultSession.webRequest.onHeadersReceived(filter, (details, callback) => {
      if (!isOpenHost(details.url)) return callback({});
      const preflight = details.method === "OPTIONS";
      callback({
        responseHeaders: openCorsHeaders(details.responseHeaders),
        ...(preflight ? { statusLine: "HTTP/1.1 200 OK" } : {}),
      });
    });

    ipcMain.handle("app:version", () => app.getVersion());
    ipcMain.handle("soundcloud:client-id", (_event, force: boolean) => getSoundCloudClientId(Boolean(force)));

    createWindow();
    setupUpdates(() => mainWindow);
  });

  app.on("window-all-closed", () => {
    if (process.platform !== "darwin") app.quit();
  });
}
