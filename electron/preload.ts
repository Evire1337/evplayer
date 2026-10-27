import { contextBridge, ipcRenderer, webUtils, type IpcRendererEvent } from "electron";
import type { UpdateState } from "./updates";
import type { LocalTrackEdit, LocalTrackRecord } from "./local-files";
import type { TrayOptions } from "./tray";
import type { DiscordUser, PresenceState } from "./discord";
import type { OverlayOptions } from "./overlay";
import type { SpotifyTokens } from "./spotifyAuth";

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
  onPlayerCommand: (cb: (action: string, value?: number) => void) => {
    const listener = (_e: IpcRendererEvent, action: string, value?: number) => cb(action, value);
    ipcRenderer.on("player:command", listener);
    return () => ipcRenderer.removeListener("player:command", listener);
  },
  showWindow: () => ipcRenderer.send("window:show"),
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
  /** accelerator -> command; resolves with the keys another program already holds */
  setGlobalShortcuts: (map: Record<string, string>): Promise<string[]> => ipcRenderer.invoke("shortcuts:global", map),
  overlay: {
    setOptions: (options: Partial<OverlayOptions>) => ipcRenderer.send("overlay:set-options", options),
    toggle: () => ipcRenderer.send("overlay:toggle"),
    hide: () => ipcRenderer.send("overlay:hide"),
    /** main window -> mini player */
    send: (state: unknown) => ipcRenderer.send("overlay:state", state),
    onVisible: (cb: (visible: boolean) => void) => on("overlay:visible", cb),
    onReady: (cb: () => void) => on("overlay:ready", cb),
    // used inside the mini player window
    ready: () => ipcRenderer.send("overlay:ready"),
    onState: (cb: (state: unknown) => void) => on("overlay:state", cb),
    command: (action: string, value?: number) => ipcRenderer.send("overlay:command", action, value),
  },
  spotify: {
    signIn: (clientId: string): Promise<SpotifyTokens> => ipcRenderer.invoke("spotify:sign-in", clientId),
    refresh: (clientId: string, refreshToken: string): Promise<SpotifyTokens> => ipcRenderer.invoke("spotify:refresh", clientId, refreshToken),
  },
  discord: {
    setAppId: (appId: string | null) => ipcRenderer.send("discord:set", appId),
    update: (state: PresenceState | null) => ipcRenderer.send("discord:update", state),
    status: (): Promise<{ status: "off" | "connecting" | "connected" | "no-discord" | "error"; user: DiscordUser | null; error: string | null }> => ipcRenderer.invoke("discord:status"),
  },
};

export type EvPlayerApi = typeof api;
export type { LocalTrackRecord, LocalTrackEdit, DiscordUser };
contextBridge.exposeInMainWorld("evplayer", api);
