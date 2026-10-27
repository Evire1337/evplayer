import type { EvPlayerApi } from "../electron/preload";

declare global {
  interface Window {
    evplayer?: EvPlayerApi;
  }
}

export {};
