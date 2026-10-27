import { discordAppId, useSettings } from "@/settings/store";
import { currentTrack, usePlayer } from "@/player/store";
import { artistLine } from "@/ui/format";
import { globalMap } from "@/settings/shortcuts";
import { useLibrary } from "@/library/store";
import { runAction, type Command } from "./actions";
import { useLyricsLine } from "@/ui/lyricsLine";
import { sendPhoneMedia } from "@/platform/androidMedia";

// Keeps the main process in step with the page: tray menu and options, global keys,
// Discord status. Also takes commands from the tray and the global keys.

export function startBridge(): void {
  const api = window.evplayer;
  if (!api) return;

  const pushSettings = () => {
    const s = useSettings.getState();
    api.tray.setOptions({ closeToTray: s.closeToTray, minimizeToTray: s.minimizeToTray, locale: "ru" });
    api.setGlobalShortcuts(s.globalShortcuts ? globalMap(s.shortcuts) : {});
    api.discord.setAppId(s.discordEnabled && discordAppId(s) ? discordAppId(s) : null);
    api.overlay.setOptions({ enabled: s.overlayEnabled, onTop: s.overlayOnTop, onMinimize: s.overlayOnMinimize, corner: s.overlayCorner });
  };
  pushSettings();
  useSettings.subscribe((s, prev) => {
    const keys: Array<keyof typeof s> = [
      "closeToTray", "minimizeToTray", "globalShortcuts", "shortcuts", "discordEnabled", "discordAppId",
      "overlayEnabled", "overlayOnTop", "overlayOnMinimize", "overlayCorner",
    ];
    if (keys.some((k) => s[k] !== prev[k])) pushSettings();
    const discordKeys: Array<keyof typeof s> = ["discordEnabled", "discordShowArtwork", "discordShowPaused", "discordShowTimeline", "discordShowButton", "discordStatusShows"];
    if (discordKeys.some((k) => s[k] !== prev[k])) pushPresence(true);
    if (s.overlayShowLyrics !== prev.overlayShowLyrics) pushOverlay();
    if (s.noCache !== prev.noCache) void api.setNoCache(s.noCache);
  });
  // the flag file can be missing (a fresh install restored from a backup): write it again
  if (useSettings.getState().noCache) void api.setNoCache(true);

  api.onPlayerCommand((action, value) => runAction(action as Command, value));

  let lastTray = "";
  let lastPresence = "";
  let lastStatus = "";
  const pushPresence = (force = false) => {
    const s = usePlayer.getState();
    const track = currentTrack(s);
    const playing = s.status === "playing";
    const key = `${track?.id}|${playing}`;
    // a jump in position (seek) also needs a fresh "elapsed" time in Discord
    if (!force && key === lastPresence) return;
    lastPresence = key;
    const o = useSettings.getState();
    if (!track || !o.discordEnabled || (!playing && !o.discordShowPaused)) {
      api.discord.update(null);
      return;
    }
    const startedAt = Date.now() - s.positionMs;
    api.discord.update({
      title: track.title,
      artist: artistLine(track.artists),
      artwork: o.discordShowArtwork ? track.artwork : undefined,
      url: o.discordShowButton ? track.permalink : undefined,
      paused: !playing,
      startedAt: o.discordShowTimeline ? startedAt : undefined,
      endsAt: o.discordShowTimeline && s.durationMs ? startedAt + s.durationMs : undefined,
      statusShows: o.discordStatusShows,
    });
  };

  // the mini player window gets a small copy of the player state
  let lastOverlay = "";
  const pushOverlay = () => {
    const s = usePlayer.getState();
    const track = currentTrack(s);
    const state = {
      track: track ? { id: track.id, title: track.title, artist: artistLine(track.artists), artwork: track.artwork } : null,
      status: s.status,
      positionMs: Math.round(s.positionMs / 500) * 500,
      durationMs: s.durationMs,
      liked: track ? useLibrary.getState().likes.some((l) => l.track.id === track.id) : false,
      line: useSettings.getState().overlayShowLyrics ? useLyricsLine.getState().line : null,
    };
    const key = JSON.stringify(state);
    if (key === lastOverlay) return;
    lastOverlay = key;
    api.overlay.send(state);
  };
  usePlayer.subscribe(pushOverlay);
  useLibrary.subscribe(pushOverlay);
  useLyricsLine.subscribe(pushOverlay);
  api.overlay.onReady(() => {
    lastOverlay = "";
    pushOverlay();
  });

  // phone: the song in the notification and on the lock screen. Position is only sent when
  // it jumps (seek, new song, play/pause); the phone moves the bar on by itself in between
  const pushPhone = () => {
    const s = usePlayer.getState();
    const track = currentTrack(s);
    if (!track) return sendPhoneMedia(null);
    sendPhoneMedia({
      title: track.title,
      artist: artistLine(track.artists),
      artwork: track.artwork && /^https:/.test(track.artwork) ? track.artwork : undefined,
      playing: s.status === "playing",
      positionMs: Math.round(s.positionMs / 2000) * 2000,
      durationMs: Math.round(s.durationMs),
      liked: useLibrary.getState().likes.some((l) => l.track.id === track.id),
    });
  };
  let phonePos = 0;
  usePlayer.subscribe((s, prev) => {
    const jumped = Math.abs(s.positionMs - phonePos) > 4000;
    if (s.status !== prev.status || currentTrack(s)?.id !== currentTrack(prev)?.id || s.durationMs !== prev.durationMs || jumped) {
      phonePos = s.positionMs;
      pushPhone();
    } else if (s.status === "playing") {
      phonePos = s.positionMs;
    }
  });
  useLibrary.subscribe(pushPhone);

  usePlayer.subscribe((s, prev) => {
    const track = currentTrack(s);
    const trayKey = `${s.status}|${track?.id}`;
    if (trayKey !== lastTray) {
      lastTray = trayKey;
      api.tray.update({ status: s.status, track: track ? { title: track.title, artist: artistLine(track.artists) } : null });
    }
    const seeked = Math.abs(s.positionMs - prev.positionMs) > 3000 && s.status === "playing";
    if (s.status !== lastStatus || seeked || currentTrack(prev)?.id !== track?.id) {
      lastStatus = s.status;
      if (s.status !== "loading") pushPresence(seeked);
    }
  });
}
