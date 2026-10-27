import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { Bindings } from "./shortcuts";

// Everything in the settings page. Saved on this computer; the parts the main process
// needs (tray, global keys, Discord, mini player) are sent to it by app/bridge.ts.

export type SearchSource = "all" | "soundcloud" | "youtube";
export type OverlayCorner = "top-left" | "top-right" | "bottom-left" | "bottom-right";
/** what Discord shows next to your name in the member list */
export type DiscordStatusShows = "app" | "artist" | "song";

/**
 * Discord needs an application id for "Listening to …". Put the id of the EvPlayer
 * application from discord.com/developers here before building, and everyone using the
 * build gets the Discord status without setting anything up.
 */
export const DEFAULT_DISCORD_APP_ID = "";

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

export interface SettingsState {
  // playback
  autoplay: boolean;
  youtubeMatchSoundCloud: boolean;
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
  soundcloudAccount: SoundCloudAccount | null;
  /** your own app from developer.spotify.com: lets imports read whole playlists */
  spotifyClientId: string;
  spotifyClientSecret: string;
  // discord
  discordEnabled: boolean;
  discordAppId: string;
  discordShowArtwork: boolean;
  discordShowPaused: boolean;
  discordShowTimeline: boolean;
  discordShowButton: boolean;
  discordStatusShows: DiscordStatusShows;
  set: (patch: Partial<Omit<SettingsState, "set" | "reset">>) => void;
  reset: () => void;
}

const DEFAULTS = {
  autoplay: true,
  youtubeMatchSoundCloud: true,
  defaultSearchSource: "all" as SearchSource,
  resumeOnStart: true,
  pauseOnDeviceChange: true,
  doubleClickToPlay: false,
  animatedBackdrop: true,
  coverGlow: true,
  lyricsBlur: true,
  lyricsSize: "m" as const,
  geniusFallback: true,
  closeToTray: true,
  minimizeToTray: false,
  globalShortcuts: true,
  shortcuts: {} as Bindings,
  overlayEnabled: true,
  overlayOnTop: true,
  overlayOnMinimize: false,
  overlayCorner: "bottom-right" as OverlayCorner,
  overlayShowLyrics: true,
  soundcloudClientId: "",
  soundcloudAccount: null as SoundCloudAccount | null,
  spotifyClientId: "",
  spotifyClientSecret: "",
  discordEnabled: false,
  discordAppId: "",
  discordShowArtwork: true,
  discordShowPaused: true,
  discordShowTimeline: true,
  discordShowButton: true,
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

export const discordAppId = (s: Pick<SettingsState, "discordAppId"> = useSettings.getState()) => s.discordAppId.trim() || DEFAULT_DISCORD_APP_ID;
