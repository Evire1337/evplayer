// The Android side of the player: the playing song is shown in the notification shade and
// on the lock screen by MediaService.kt (src-tauri/gen/android), which the page reaches as
// window.EvPlayerAndroid. Button presses there come back through
// window.__evplayerMediaCommand, and the app leaving or returning to the screen through
// window.__evplayerVisible.

interface AndroidBridge {
  update(json: string): void;
  clear(): void;
  keepsRunning?(): boolean;
  askToKeepRunning?(): void;
}

declare global {
  interface Window {
    EvPlayerAndroid?: AndroidBridge;
    __evplayerMediaCommand?: (action: string, value?: number) => void;
    __evplayerVisible?: (visible: boolean) => void;
    /** the phone's "back": true when the page went back itself */
    __evplayerBack?: () => boolean;
  }
}

const commandListeners = new Set<(action: string, value?: number) => void>();
const visibleListeners = new Set<(visible: boolean) => void>();

if (typeof window !== "undefined") {
  window.__evplayerMediaCommand = (action, value) => commandListeners.forEach((cb) => cb(action, value));
  window.__evplayerVisible = (visible) => visibleListeners.forEach((cb) => cb(visible));
}

/**
 * Android may close apps that sit in the background; with battery optimisation off for
 * EvPlayer it leaves it running. null: not on a phone (or an older APK).
 */
export function phoneKeepsRunning(): boolean | null {
  const bridge = typeof window !== "undefined" ? window.EvPlayerAndroid : undefined;
  return bridge?.keepsRunning ? bridge.keepsRunning() : null;
}

export function askPhoneToKeepRunning(): void {
  window.EvPlayerAndroid?.askToKeepRunning?.();
}

export function onPhoneCommand(cb: (action: string, value?: number) => void): () => void {
  commandListeners.add(cb);
  return () => commandListeners.delete(cb);
}

export function onPhoneVisible(cb: (visible: boolean) => void): () => void {
  visibleListeners.add(cb);
  return () => visibleListeners.delete(cb);
}

export interface PhoneMediaState {
  title: string;
  artist: string;
  artwork?: string;
  playing: boolean;
  positionMs: number;
  durationMs: number;
  liked: boolean;
}

let last = "";

/** tell the notification what is playing; repeats of the same state are skipped */
export function sendPhoneMedia(state: PhoneMediaState | null): void {
  const bridge = typeof window !== "undefined" ? window.EvPlayerAndroid : undefined;
  if (!bridge) return;
  if (!state) {
    if (last !== "") bridge.clear();
    last = "";
    return;
  }
  const json = JSON.stringify(state);
  if (json === last) return;
  last = json;
  bridge.update(json);
}
