import { contextBridge, ipcRenderer, webUtils, type IpcRendererEvent } from "electron";
import type { UpdateState } from "./updates";
import type { LocalTrackEdit, LocalTrackRecord } from "./local-files";
import type { TrayAction, TrayOptions } from "./tray";
import type { PresenceState } from "./discord";

function on<T>(channel: string, cb: (value: T) => void): () => void {
  const listener = (_e: IpcRendererEvent, value: T) => cb(value);
  ipcRenderer.on(channel, listener);
  return () => ipcRenderer.removeListener(channel, listener);
}

type AddResult = { added: LocalTrackRecord[]; skipped: number };

const api = {
  version: (): Promise<string> => ipcRenderer.invoke("app:version"),
  soundcloudClientId: (force = false): Promise<string> => ipcRenderer.invoke("soundcloud:client-id", force),
  cacheSize: (): Promise<{ cache: number }> => ipcRenderer.invoke("app:data-size"),
  clearCache: (): Promise<void> => ipcRenderer.invoke("app:clear-cache"),
  openDataFolder: (): Promise<void> => ipcRenderer.invoke("app:open-data-folder"),
  onPlayerCommand: (cb: (action: TrayAction) => void) => on("player:command", cb),
  onWindowVisible: (cb: (visible: boolean) => void) => on("window:visible", cb),
  updates: {
    get: (): Promise<UpdateState> => ipcRenderer.invoke("updates:get"),
    check: (): Promise<void> => ipcRenderer.invoke("updates:check"),
    install: (): Promise<void> => ipcRenderer.invoke("updates:install"),
    onState: (cb: (state: UpdateState) => void) => on("updates:state", cb),
  },
  localFiles: {
    list: (): Promise<LocalTrackRecord[]> => ipcRenderer.invoke("local:list"),
    pick: (kind: "files" | "folder"): Promise<AddResult> => ipcRenderer.invoke("local:pick", kind),
    /** files dropped onto the window */
    addDropped: (files: File[]): Promise<AddResult> =>
      ipcRenderer.invoke("local:add-paths", files.map((f) => webUtils.getPathForFile(f)).filter(Boolean)),
    remove: (ids: string[]): Promise<void> => ipcRenderer.invoke("local:remove", ids),
    reveal: (id: string): Promise<void> => ipcRenderer.invoke("local:reveal", id),
    update: (id: string, patch: LocalTrackEdit): Promise<LocalTrackRecord | undefined> => ipcRenderer.invoke("local:update", id, patch),
    watchedFolders: (): Promise<string[]> => ipcRenderer.invoke("local:watched-folders"),
    unwatchFolder: (folder: string): Promise<void> => ipcRenderer.invoke("local:unwatch-folder", folder),
    onChanged: (cb: () => void) => on("local:changed", cb),
  },
  files: {
    saveText: (name: string, text: string): Promise<boolean> => ipcRenderer.invoke("file:save-text", name, text),
    openText: (extensions: string[]): Promise<Array<{ name: string; text: string }>> => ipcRenderer.invoke("file:open-text", extensions),
  },
  tray: {
    setOptions: (options: Partial<TrayOptions>) => ipcRenderer.send("tray:set-options", options),
    update: (state: { status?: string; track?: { title?: string; artist?: string } | null }) => ipcRenderer.send("tray:update", state),
  },
  setGlobalShortcuts: (enabled: boolean) => ipcRenderer.send("shortcuts:global", enabled),
  discord: {
    setAppId: (appId: string | null) => ipcRenderer.send("discord:set", appId),
    update: (state: PresenceState | null) => ipcRenderer.send("discord:update", state),
    status: (): Promise<"off" | "connecting" | "connected" | "no-discord"> => ipcRenderer.invoke("discord:status"),
  },
};

export type EvPlayerApi = typeof api;
export type { LocalTrackRecord, LocalTrackEdit };
contextBridge.exposeInMainWorld("evplayer", api);
