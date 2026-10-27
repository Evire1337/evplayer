import { contextBridge, ipcRenderer, type IpcRendererEvent } from "electron";
import type { UpdateState } from "./updates";

const api = {
  version: (): Promise<string> => ipcRenderer.invoke("app:version"),
  soundcloudClientId: (force = false): Promise<string> => ipcRenderer.invoke("soundcloud:client-id", force),
  updates: {
    get: (): Promise<UpdateState> => ipcRenderer.invoke("updates:get"),
    check: (): Promise<void> => ipcRenderer.invoke("updates:check"),
    install: (): Promise<void> => ipcRenderer.invoke("updates:install"),
    onState: (cb: (state: UpdateState) => void): (() => void) => {
      const listener = (_e: IpcRendererEvent, state: UpdateState) => cb(state);
      ipcRenderer.on("updates:state", listener);
      return () => ipcRenderer.removeListener("updates:state", listener);
    },
  },
};

export type EvPlayerApi = typeof api;
contextBridge.exposeInMainWorld("evplayer", api);
