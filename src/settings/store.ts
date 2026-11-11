import type { Language } from "@/i18n/lang";
import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { Bindings } from "./shortcuts";

// Everything in the settings page. Saved on this computer; the parts the main process
// needs (tray, global keys, Discord, mini player) are sent to it by app/bridge.ts.

export type SearchSource = "all" | "soundcloud" | "youtube";
export type OverlayCorner = "top-left" | "top-center" | "top-right" | "bottom-left" | "bottom-center" | "bottom-right";
/** what Discord shows next to your name in the member list */
export type DiscordStatusShows = "app" | "artist" | "song";

/**
 * EvPlayer's own application id from discord.com/developers, where it is named "EvPlayer".
 * Discord always needs an application id for "Listening to …"; shipping ours here means
 * users just launch the player and the status appears, with nothing to set up.
 * Never paste another project's id here: the status would show that project's name.
 */
export const DEFAULT_DISCORD_APP_ID = "1564485226267279360";

/**
 * Client ID of EvPlayer's app on developer.spotify.com (no secret needed: sign-in uses
 * PKCE). With it, "Sign in with Spotify" works without any setup. Its redirect URI must be
 * http://127.0.0.1:43821/callback.
 */
export const DEFAULT_SPOTIFY_CLIENT_ID = "";

export interface SpotifyAccount {
  name: string;
  avatar?: string;
  product?: string;
  accessToken: string;
  refreshToken: string;
  expiresAt: number;
}

export interface SoundCloudAccount {
  id: string;
  name: string;
  avatar?: string;
  permalink: string;
  followers?: number;
  likes?: number;
  tracks?: number;
  /** keep the likes in step: new likes on SoundCloud show up here on start */
  syncLikes: boolean;
  lastSync?: string;
}

/** signed in to SoundCloud through its official OAuth: this is what lets likes and playlists be written */
export interface SoundCloudLogin {
  id: string;
  name: string;
  avatar?: string;
  accessToken: string;
  refreshToken: string;
  expiresAt: number;
}

export interface SettingsState {
  // playback
  autoplay: boolean;
  /** seconds the next song fades in over the end of the current one; 0 = straight cut, no gap */
  crossfade: number;
  defaultSearchSource: SearchSource;
  /** start where you stopped last time (queue and position) */
  resumeOnStart: boolean;
  /** pause when headphones are unplugged or the output device changes */
  pauseOnDeviceChange: boolean;
  /** double click on a track plays it, single click only selects */
  doubleClickToPlay: boolean;
  // appearance
  animatedBackdrop: boolean;
  coverGlow: boolean;
  lyricsBlur: boolean;
  lyricsSize: "s" | "m" | "l";
  /** memory saver: no live backdrop, glow, blur or animations; the page unloads sooner when hidden */
  memorySaver: boolean;
  // "don't keep" list (memory saver card): each one stops one kind of remembered data
  /** covers and pictures: no browser disk cache (from the next start) */
  noCache: boolean;
  /** answers of the services: recommendations, similar songs, lyrics are fetched each time */
  noServiceCache: boolean;
  /** remembered loudness of each song for "level the volume" */
  noLoudnessMemory: boolean;
  /** listening statistics: tops, charts and smart playlists stop growing */
  noStats: boolean;
  /** "Recently played" on the home page */
  noHistory: boolean;
  /** the "Up next" panel on the now playing screen */
  showQueue: boolean;
  // library
  libraryView: "grid" | "list";
  trackSort: { key: import("@/library/sort").SortKey; dir: import("@/library/sort").SortDir };
  // lyrics
  geniusFallback: boolean;
  // window
  closeToTray: boolean;
  minimizeToTray: boolean;
  globalShortcuts: boolean;
  shortcuts: Bindings;
  // mini player
  overlayEnabled: boolean;
  overlayOnTop: boolean;
  overlayOnMinimize: boolean;
  overlayCorner: OverlayCorner;
  overlayShowLyrics: boolean;
  // services
  /** client id of your own app from soundcloud.com/you/apps; empty = automatic */
  soundcloudClientId: string;
  /** the interface language: the system's, or one chosen (see src/i18n) */
  language: Language;
  /** keep SoundCloud out of reach from the countries below (see services/region.ts) */
  regionGuard: boolean;
  /** two-letter codes of the countries SoundCloud is not asked from */
  regionCountries: string[];
  soundcloudAccount: SoundCloudAccount | null;
  /** secret of that same app (needed for the OAuth sign-in) */
  soundcloudClientSecret: string;
  soundcloudLogin: SoundCloudLogin | null;
  /** full sync: likes and playlists you change here are changed on your SoundCloud too */
  soundcloudFullSync: boolean;
  /** your own app from developer.spotify.com: lets imports read whole playlists */
  spotifyClientId: string;
  spotifyClientSecret: string;
  /** signed in with Spotify (PKCE): playlists of any length, your liked songs */
  spotifyAccount: SpotifyAccount | null;
  /** a Yandex Music profile by login: its public playlists and likes can be imported */
  yandexAccount: { login: string; name: string; playlists: number } | null;
  // discord
  discordEnabled: boolean;
  discordAppId: string;
  discordShowArtwork: boolean;
  discordShowPaused: boolean;
  discordShowTimeline: boolean;
  discordShowButton: boolean;
  /** friends may press "Ask to join" on the status to listen along */
  discordJoin: boolean;
  /** the developer's tools (Настройки → Диагностика): opened by seven clicks on the version */
  devTools: boolean;
  discordStatusShows: DiscordStatusShows;
  set: (patch: Partial<Omit<SettingsState, "set" | "reset">>) => void;
  reset: () => void;
}

/** the settings were saved by an earlier run of the app */
function existingInstall(): boolean {
  try {
    return localStorage.getItem("evplayer.settings.v1") !== null;
  } catch {
    return false;
  }
}

const DEFAULTS = {
  autoplay: true,
  defaultSearchSource: "all" as SearchSource,
  crossfade: 0,
  resumeOnStart: true,
  pauseOnDeviceChange: true,
  doubleClickToPlay: false,
  animatedBackdrop: true,
  coverGlow: true,
  lyricsBlur: true,
  lyricsSize: "m" as const,
  geniusFallback: true,
  showQueue: true,
  memorySaver: false,
  noCache: false,
  noServiceCache: false,
  noLoudnessMemory: false,
  noStats: false,
  noHistory: false,
  libraryView: "grid" as "grid" | "list",
  trackSort: { key: "custom", dir: "asc" } as { key: import("@/library/sort").SortKey; dir: import("@/library/sort").SortDir },
  closeToTray: true,
  minimizeToTray: false,
  globalShortcuts: true,
  shortcuts: {} as Bindings,
  overlayEnabled: true,
  overlayOnTop: true,
  overlayOnMinimize: true,
  overlayCorner: "top-center" as OverlayCorner,
  overlayShowLyrics: true,
  soundcloudClientId: "",
  // an app that was in use before there was a choice stays as it was (Russian); a new one follows the system
  language: (existingInstall() ? "ru" : "auto") as Language,
  // off: nothing changes for anyone until they ask for it (SoundCloud blocks by country only for some)
  regionGuard: false,
  regionCountries: ["RU"],
  soundcloudAccount: null as SoundCloudAccount | null,
  soundcloudClientSecret: "",
  soundcloudLogin: null as SoundCloudLogin | null,
  soundcloudFullSync: false,
  spotifyClientId: "",
  spotifyClientSecret: "",
  spotifyAccount: null as SpotifyAccount | null,
  yandexAccount: null as { login: string; name: string; playlists: number } | null,
  discordEnabled: true,
  discordAppId: "",
  discordShowArtwork: true,
  discordShowPaused: true,
  discordShowTimeline: true,
  discordShowButton: true,
  discordJoin: true,
  devTools: false,
  discordStatusShows: "song" as DiscordStatusShows,
};

export const useSettings = create<SettingsState>()(
  persist(
    (set) => ({
      ...DEFAULTS,
      set: (patch) => set(patch),
      reset: () => set(DEFAULTS),
    }),
    { name: "evplayer.settings.v1", storage: createJSONStorage(() => localStorage), version: 1 },
  ),
);

/** is this kind of data kept, by the "don't keep" list */
export function keeps(kind: "serviceCache" | "loudness" | "stats" | "history"): boolean {
  const s = useSettings.getState();
  return !{ serviceCache: s.noServiceCache, loudness: s.noLoudnessMemory, stats: s.noStats, history: s.noHistory }[kind];
}

export const discordAppId = (s: Pick<SettingsState, "discordAppId"> = useSettings.getState()) => s.discordAppId.trim() || DEFAULT_DISCORD_APP_ID;

export const spotifyClientId = (s: Pick<SettingsState, "spotifyClientId"> = useSettings.getState()) => s.spotifyClientId.trim() || DEFAULT_SPOTIFY_CLIENT_ID;
