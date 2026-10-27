import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";

// Everything in the settings page. Saved on this computer; the parts the main process
// needs (tray, global keys, Discord) are sent to it by settings/bridge.ts.

export type SearchSource = "all" | "soundcloud" | "youtube";

export interface SettingsState {
  // playback
  autoplay: boolean;
  youtubeMatchSoundCloud: boolean;
  defaultSearchSource: SearchSource;
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
  /** client id of your own app from soundcloud.com/you/apps; empty = automatic */
  soundcloudClientId: string;
  // discord
  discordEnabled: boolean;
  discordAppId: string;
  set: (patch: Partial<Omit<SettingsState, "set" | "reset">>) => void;
  reset: () => void;
}

const DEFAULTS = {
  autoplay: true,
  youtubeMatchSoundCloud: true,
  defaultSearchSource: "all" as SearchSource,
  animatedBackdrop: true,
  coverGlow: true,
  lyricsBlur: true,
  lyricsSize: "m" as const,
  geniusFallback: true,
  closeToTray: true,
  minimizeToTray: false,
  globalShortcuts: true,
  soundcloudClientId: "",
  discordEnabled: false,
  discordAppId: "",
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
