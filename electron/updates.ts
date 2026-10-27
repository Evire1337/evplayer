import { app, ipcMain, type BrowserWindow } from "electron";
import electronUpdater from "electron-updater";

// Updates come from the GitHub releases of this repository. A new version downloads in the
// background; the app offers a restart, and installs on the next quit if the user waits.

const { autoUpdater } = electronUpdater;
const CHECK_EVERY_MS = 4 * 60 * 60 * 1000;

export type UpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "downloading"; version: string; percent: number }
  | { status: "ready"; version: string }
  | { status: "latest" }
  | { status: "error"; message: string };

/** a short, readable reason instead of electron-updater's full HTTP dump */
export function shortUpdateError(message: string): string {
  if (/latest\.yml/i.test(message) && /404/.test(message)) return "в последнем релизе на GitHub нет файла обновления (latest.yml)";
  if (/ENOTFOUND|ECONNREFUSED|ETIMEDOUT|net::ERR/i.test(message)) return "нет связи с GitHub";
  if (/403|rate limit/i.test(message)) return "GitHub временно ограничил запросы, попробуйте позже";
  const first = message.split(/\n|(?<=\.)\s/)[0] ?? message;
  return first.length > 140 ? `${first.slice(0, 137)}…` : first;
}

export function setupUpdates(getWindow: () => BrowserWindow | null): void {
  let state: UpdateState = { status: "idle" };
  const send = (next: UpdateState) => {
    state = next;
    const win = getWindow();
    if (win && !win.isDestroyed()) win.webContents.send("updates:state", state);
  };

  ipcMain.handle("updates:get", () => state);
  ipcMain.handle("updates:install", () => {
    if (state.status === "ready") autoUpdater.quitAndInstall(true, true);
  });
  ipcMain.handle("updates:check", async () => {
    if (!app.isPackaged) {
      send({ status: "latest" });
      return;
    }
    await autoUpdater.checkForUpdates().catch((err: Error) => send({ status: "error", message: shortUpdateError(err.message) }));
  });

  if (!app.isPackaged) return;
  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = true;
  autoUpdater.on("checking-for-update", () => send({ status: "checking" }));
  autoUpdater.on("update-available", (info) => send({ status: "downloading", version: info.version, percent: 0 }));
  autoUpdater.on("download-progress", (p) =>
    send({ status: "downloading", version: state.status === "downloading" ? state.version : "", percent: Math.round(p.percent) }),
  );
  autoUpdater.on("update-downloaded", (info) => send({ status: "ready", version: info.version }));
  autoUpdater.on("update-not-available", () => send({ status: "latest" }));
  autoUpdater.on("error", (err) => send({ status: "error", message: shortUpdateError(err?.message ?? String(err)) }));

  const check = () => void autoUpdater.checkForUpdates().catch(() => {});
  setTimeout(check, 10_000);
  setInterval(check, CHECK_EVERY_MS);
}
