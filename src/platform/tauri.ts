import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { EvPlayerApi, UpdateState } from "./api";
import { installProxy, schemeBase } from "./net";
import { onPhoneCommand, onPhoneVisible } from "./androidMedia";

// `window.evplayer` on top of Tauri: every call is a command in src-tauri/src/lib.rs, every
// subscription an event the Rust side sends to this window.

function on<T>(event: string, cb: (payload: T) => void): () => void {
  let off: (() => void) | null = null;
  let cancelled = false;
  void getCurrentWebviewWindow()
    .listen<T>(event, (e) => cb(e.payload))
    .then((unlisten) => {
      if (cancelled) unlisten();
      else off = unlisten;
    });
  return () => {
    cancelled = true;
    off?.();
  };
}

const fire = (cmd: string, args?: Record<string, unknown>) => void invoke(cmd, args).catch((e) => console.warn(`[${cmd}]`, e));

export function createTauriApi(): EvPlayerApi {
  const media = schemeBase("evmedia");
  const win = getCurrentWebviewWindow();
  return {
    version: () => getVersion(),
    soundcloudClientId: (force = false) => invoke("soundcloud_client_id", { force }),
    cacheSize: () => invoke("cache_size"),
    clearCache: () => invoke("clear_cache"),
    setNoCache: (on) => invoke("set_no_cache", { on }),
    openDataFolder: () => invoke("open_data_folder"),
    onPlayerCommand: (cb) => {
      const offDesktop = on<{ action: string; value?: number | null }>("player:command", (p) => cb(p.action, p.value ?? undefined));
      const offPhone = onPhoneCommand(cb);
      return () => {
        offDesktop();
        offPhone();
      };
    },
    showWindow: () => fire("show_window"),
    onWindowVisible: (cb) => {
      const offDesktop = on<boolean>("window:visible", cb);
      const offPhone = onPhoneVisible(cb);
      return () => {
        offDesktop();
        offPhone();
      };
    },
    window: {
      minimize: () => void win.minimize(),
      toggleMaximize: () => void win.toggleMaximize(),
      close: () => void win.close(),
      onMaximized: (cb) => {
        const check = () => void win.isMaximized().then(cb);
        check();
        let off: (() => void) | null = null;
        void win.onResized(check).then((u) => (off = u));
        return () => off?.();
      },
    },
    updates: {
      get: () => invoke<UpdateState>("updates_get"),
      check: () => invoke("updates_check"),
      install: () => invoke("updates_install"),
      onState: (cb) => on<UpdateState>("updates:state", cb),
    },
    localFiles: {
      list: () => invoke("local_list"),
      pick: (kind) => invoke("local_pick", { kind }),
      addPaths: (paths) => invoke("local_add_paths", { paths }),
      onDrop: (cb) => on<string[]>("files:drop", cb),
      onDragging: (cb) => on<boolean>("files:drag", cb),
      remove: (ids) => invoke("local_remove", { ids }),
      reveal: (id) => invoke("local_reveal", { id }),
      update: (id, patch) => invoke("local_update", { id, patch }),
      watchedFolders: () => invoke("local_watched_folders"),
      unwatchFolder: (folder) => invoke("local_unwatch_folder", { folder }),
      onChanged: (cb) => on<null>("local:changed", () => cb()),
      // songs come from the media server (every engine can play from it), covers from evmedia
      mediaUrl: (kind, id) => (kind === "track" && window.__EVPLAYER_MEDIA__ ? `${window.__EVPLAYER_MEDIA__}/track/${encodeURIComponent(id)}` : media + encodeURIComponent(`${kind}/${id}`)),
    },
    files: {
      saveText: (name, text) => invoke("save_text", { name, text }),
      openText: (extensions) => invoke("open_text", { extensions }),
    },
    tray: {
      setOptions: (options) => fire("tray_set_options", { options }),
      update: (state) => fire("tray_update", { state }),
    },
    setGlobalShortcuts: (map) => invoke("set_global_shortcuts", { map }),
    overlay: {
      setOptions: (options) => fire("overlay_set_options", { options }),
      toggle: () => fire("overlay_toggle"),
      hide: () => fire("overlay_hide"),
      send: (state) => fire("overlay_state", { state }),
      onVisible: (cb) => on<boolean>("overlay:visible", cb),
      onReady: (cb) => on<null>("overlay:ready", () => cb()),
      ready: () => fire("overlay_ready"),
      onState: (cb) => on<unknown>("overlay:state", cb),
      command: (action, value) => fire("overlay_command", { action, value: value ?? null }),
    },
    spotify: {
      signIn: (clientId) => invoke("spotify_sign_in", { clientId }),
      refresh: (clientId, refreshToken) => invoke("spotify_refresh", { clientId, refreshToken }),
    },
    discord: {
      setAppId: (appId) => fire("discord_set", { appId }),
      update: (presence) => fire("discord_update", { presence }),
      status: () => invoke("discord_status"),
    },
    migrateFromElectron: () => invoke("migrate_electron"),
  };
}

export function installTauri(): void {
  installProxy();
  window.evplayer = createTauriApi();
}
