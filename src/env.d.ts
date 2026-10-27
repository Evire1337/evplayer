import type { EvPlayerApi } from "./platform/api";

declare global {
  interface Window {
    evplayer?: EvPlayerApi;
  }
}

export {};
