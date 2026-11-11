import type { UpdateState } from "./api";

// Phone updates: the app looks at the newest GitHub release; if it is newer than this one
// and has an APK, "Скачать" downloads it inside the app (UpdateInstaller.kt) and opens
// Android's installer, which puts it over this one (same signing key, so everything stays).
// An older APK without that part opens the APK in the browser instead.

const LATEST = "https://api.github.com/repos/Evire1337/evplayer/releases/latest";
const EVERY_MS = 4 * 60 * 60 * 1000;

let state: UpdateState = { status: "idle" };
let apkUrl: string | null = null;
let apkVersion = "";

declare global {
  interface Window {
    __evplayerUpdateProgress?: (percent: number, finished: boolean, error: string | null) => void;
  }
}
const listeners = new Set<(s: UpdateState) => void>();

function set(next: UpdateState): void {
  state = next;
  listeners.forEach((cb) => cb(next));
}

/** "0.5.1" > "0.5.0" */
export function isNewer(candidate: string, current: string): boolean {
  const a = candidate.replace(/^v/, "").split(".").map(Number);
  const b = current.replace(/^v/, "").split(".").map(Number);
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const d = (a[i] ?? 0) - (b[i] ?? 0);
    if (d) return d > 0;
  }
  return false;
}

async function check(currentVersion: () => Promise<string>): Promise<void> {
  set({ status: "checking" });
  try {
    const res = await fetch(LATEST, { headers: { accept: "application/vnd.github+json" } });
    if (!res.ok) throw new Error(res.status === 403 ? "GitHub временно ограничил запросы, попробуйте позже" : `GitHub ответил ${res.status}`);
    const release = (await res.json()) as { tag_name: string; assets: Array<{ name: string; browser_download_url: string }> };
    const apk = release.assets.find((a) => a.name.toLowerCase().endsWith(".apk"));
    const version = release.tag_name.replace(/^v/, "");
    if (apk && isNewer(version, await currentVersion())) {
      apkUrl = apk.browser_download_url;
      apkVersion = version;
      set({ status: "ready", version });
    } else {
      set({ status: "latest" });
    }
  } catch (error) {
    set({ status: "error", message: error instanceof Error ? error.message : "нет связи с GitHub" });
  }
}

export function phoneUpdates(currentVersion: () => Promise<string>, open: (url: string) => Promise<void>) {
  setTimeout(() => void check(currentVersion), 15_000);
  setInterval(() => void check(currentVersion), EVERY_MS);
  return {
    get: async () => state,
    check: () => check(currentVersion),
    install: async () => {
      if (!apkUrl) {
        // nothing to download yet (the check has not found a newer version): say so, do not do nothing
        set({ status: "error", message: "Нет ссылки на обновление: проверьте обновления ещё раз" });
        return;
      }
      const bridge = window.EvPlayerAndroid;
      if (!bridge?.installUpdate) {
        await open(apkUrl);
        return;
      }
      set({ status: "downloading", version: apkVersion, percent: 0 });
      window.__evplayerUpdateProgress = (percent, finished, error) => {
        if (error) set({ status: "error", message: error });
        else if (finished) set({ status: "ready", version: apkVersion });
        else set({ status: "downloading", version: apkVersion, percent });
      };
      bridge.installUpdate(apkUrl);
    },
    onState: (cb: (s: UpdateState) => void) => {
      listeners.add(cb);
      return () => void listeners.delete(cb);
    },
  };
}
