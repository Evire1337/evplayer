import { seedDemo } from "./demoData";
import { installProxy } from "./net";

// The page opened in a plain browser during development (`pnpm web`): there is no desktop side,
// so the dev server stands in for the part of it that the services need (tools/webBridge.ts):
// requests to the services and song streams go through it, and the SoundCloud client id comes
// from it. Everything else of `window.evplayer` stays absent, as it is in the tests: tray,
// keys, files, Discord and the updater are the desktop's.

declare global {
  interface Window {
    /** set where the page runs against the dev server's bridge, not the desktop app */
    __EVPLAYER_WEB__?: boolean;
  }
}

/** whether the dev server's bridge is behind this page */
export const isWebDev = (): boolean => typeof window !== "undefined" && window.__EVPLAYER_WEB__ === true;

export function installWeb(): void {
  window.__EVPLAYER_WEB__ = true;
  window.__EVPLAYER_MEDIA__ = `${location.origin}/__ev/media`;
  installProxy(`${location.origin}/__ev/proxy/`);
  // ?demo: sample library and history, to look at the design (platform/demoData.ts)
  seedDemo();
}

/** the SoundCloud client id, from the dev server */
export async function webClientId(force = false): Promise<string> {
  const res = await fetch(`/__ev/client-id${force ? "?force=1" : ""}`);
  if (!res.ok) throw new Error(`SoundCloud client id: ${await res.text()}`);
  return res.text();
}
