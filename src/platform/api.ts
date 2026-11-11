// What the page may ask of the desktop side. The app only ever uses `window.evplayer`; how
// it is provided (Tauri today, maybe a phone later) lives in this folder.

export type UpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "downloading"; version: string; percent: number }
  | { status: "ready"; version: string }
  | { status: "latest" }
  | { status: "error"; message: string };

export interface LocalTrackRecord {
  id: string;
  path: string;
  title: string;
  artists: string[];
  album?: string;
  durationMs: number;
  codec?: string;
  lossless?: boolean;
  sampleRate?: number;
  bitsPerSample?: number;
  bitrate?: number;
  /** the browser engine cannot decode it (ALAC, for example) */
  unsupported?: boolean;
  hasCover?: boolean;
  /** tags were changed in EvPlayer: rescans keep them instead of re-reading the file */
  edited?: boolean;
  size: number;
  addedAt: string;
}

export interface LocalTrackEdit {
  title?: string;
  artists?: string[];
  album?: string | null;
  /** new cover as a data: url, or null to drop the cover */
  cover?: string | null;
}

export interface TrayOptions {
  closeToTray: boolean;
  minimizeToTray: boolean;
  /** language chosen in the app (the system language may differ) */
  locale?: string;
}

export type OverlayCorner = "top-left" | "top-center" | "top-right" | "bottom-left" | "bottom-center" | "bottom-right";

export interface OverlayOptions {
  enabled: boolean;
  onTop: boolean;
  onMinimize: boolean;
  corner: OverlayCorner;
}

export interface DiscordUser {
  id: string;
  username: string;
  globalName?: string;
  avatar?: string;
}

export interface PresenceState {
  title: string;
  artist: string;
  artwork?: string;
  url?: string;
  /** epoch ms when the song started (with the current position taken into account) */
  startedAt?: number;
  endsAt?: number;
  paused?: boolean;
  /** what Discord shows next to your name in the member list */
  statusShows?: "app" | "artist" | "song";
  /** a paused song: where it stands (said in words, as Discord's clock would run on) */
  positionMs?: number;
  durationMs?: number;
  /** the session this person is in ("2 of 16"); `join` is for Discord's own "Ask to join" (not
      sent: Discord shows no link buttons next to it) */
  party?: { id: string; size: number; max: number; join?: string };
  /** the "Слушать вместе" button: a page that takes a friend into the session */
  joinUrl?: string;
}

/** one of the app's processes, as the diagnostics page shows it */
export interface ProcessInfo {
  pid: number;
  name: string;
  /** bytes */
  memory: number;
  /** processor time since it started */
  cpuMs: number;
  /** what it does for WebView2: "renderer" (a page), "gpu", "browser" (the manager), "utility" */
  kind?: string;
}

/** a friend asked to listen along, or this person was let into a friend's session */
export type DiscordEvent = { kind: "request"; user: DiscordUser } | { kind: "join"; secret: string };

export interface SpotifyTokens {
  accessToken: string;
  refreshToken: string;
  expiresAt: number;
}

/** a song kept on this computer for listening offline */
export interface OfflineEntry {
  id: string;
  file: string;
  ext: string;
  size: number;
  savedAt: string;
}

/** where the desktop side fetches a song from: one file, or an HLS playlist laid end to end */
export interface OfflineSource {
  kind: "file" | "hls";
  url: string;
}

export interface OfflineProgress {
  id: string;
  done: number;
  /** 0 when the service does not say how long the file is */
  total: number;
}

export type AddResult = { added: LocalTrackRecord[]; skipped: number };
type Off = () => void;

export interface EvPlayerApi {
  version: () => Promise<string>;
  soundcloudClientId: (force?: boolean) => Promise<string>;
  cacheSize: () => Promise<{ cache: number }>;
  clearCache: () => Promise<void>;
  /** keep no cache on disk (takes full effect after a restart) */
  setNoCache: (on: boolean) => Promise<void>;
  openDataFolder: () => Promise<void>;
  onPlayerCommand: (cb: (action: string, value?: number) => void) => Off;
  showWindow: () => void;
  onWindowVisible: (cb: (visible: boolean) => void) => Off;
  /** the app's own window buttons (the window has no system frame) */
  window: {
    minimize: () => void;
    toggleMaximize: () => void;
    close: () => void;
    onMaximized: (cb: (maximized: boolean) => void) => Off;
  };
  updates: {
    get: () => Promise<UpdateState>;
    check: () => Promise<void>;
    install: () => Promise<void>;
    onState: (cb: (state: UpdateState) => void) => Off;
  };
  localFiles: {
    list: () => Promise<LocalTrackRecord[]>;
    pick: (kind: "files" | "folder") => Promise<AddResult>;
    addPaths: (paths: string[]) => Promise<AddResult>;
    /** files dragged onto the window from the file manager */
    onDrop: (cb: (paths: string[]) => void) => Off;
    onDragging: (cb: (over: boolean) => void) => Off;
    remove: (ids: string[]) => Promise<void>;
    reveal: (id: string) => Promise<void>;
    update: (id: string, patch: LocalTrackEdit) => Promise<LocalTrackRecord | undefined>;
    watchedFolders: () => Promise<string[]>;
    unwatchFolder: (folder: string) => Promise<void>;
    onChanged: (cb: () => void) => Off;
    /** where a local song or cover is served from */
    mediaUrl: (kind: "track" | "cover", id: string) => string;
  };
  offline: {
    list: () => Promise<OfflineEntry[]>;
    download: (id: string, source: OfflineSource) => Promise<OfflineEntry>;
    cancel: (id: string) => void;
    remove: (ids: string[]) => Promise<void>;
    clear: () => Promise<void>;
    openFolder: () => void;
    onProgress: (cb: (progress: OfflineProgress) => void) => Off;
    /** where a kept song is played from */
    mediaUrl: (id: string) => string | null;
  };
  files: {
    saveText: (name: string, text: string) => Promise<boolean>;
    openText: (extensions: string[]) => Promise<Array<{ name: string; text: string }>>;
  };
  tray: {
    setOptions: (options: Partial<TrayOptions>) => void;
    update: (state: { status?: string; track?: { title?: string; artist?: string } | null }) => void;
  };
  /** accelerator -> command; resolves with the keys another program already holds */
  setGlobalShortcuts: (map: Record<string, string>) => Promise<string[]>;
  overlay: {
    setOptions: (options: Partial<OverlayOptions>) => void;
    toggle: () => void;
    hide: () => void;
    /** main window -> mini player */
    send: (state: unknown) => void;
    onVisible: (cb: (visible: boolean) => void) => Off;
    onReady: (cb: () => void) => Off;
    // used inside the mini player window
    ready: () => void;
    onState: (cb: (state: unknown) => void) => Off;
    command: (action: string, value?: number) => void;
  };
  spotify: {
    signIn: (clientId: string) => Promise<SpotifyTokens>;
    refresh: (clientId: string, refreshToken: string) => Promise<SpotifyTokens>;
  };
  soundcloudAuth: {
    signIn: (clientId: string, clientSecret: string) => Promise<SpotifyTokens>;
    refresh: (clientId: string, clientSecret: string, refreshToken: string) => Promise<SpotifyTokens>;
  };
  discord: {
    setAppId: (appId: string | null) => void;
    update: (state: PresenceState | null) => void;
    status: () => Promise<{ status: "off" | "connecting" | "connected" | "no-discord" | "error"; user: DiscordUser | null; error: string | null }>;
    onEvent: (cb: (event: DiscordEvent) => void) => () => void;
    /** let a friend who asked in (true) or not */
    answer: (userId: string, accept: boolean) => void;
  };
  /** the app's processes with their memory and processor time */
  processes: () => Promise<ProcessInfo[]>;
  /** evplayer:// links: the one the app started with, then each one opened while it runs */
  links: { first: () => Promise<string | null>; onOpen: (cb: (url: string) => void) => () => void };
  /** the saved state of the Electron version (read once, on the first start) */
  migrateFromElectron: () => Promise<Record<string, string> | null>;
}
