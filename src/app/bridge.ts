import { useSettings } from "@/settings/store";
import { currentTrack, usePlayer } from "@/player/store";
import { player } from "@/player/engine";
import { artistLine } from "@/ui/format";

// Keeps the main process in step with the page: tray menu and options, global keys,
// Discord status. Also takes commands from the tray and the global keys.

export function startBridge(): void {
  const api = window.evplayer;
  if (!api) return;

  const pushSettings = () => {
    const s = useSettings.getState();
    api.tray.setOptions({ closeToTray: s.closeToTray, minimizeToTray: s.minimizeToTray, locale: "ru" });
    api.setGlobalShortcuts(s.globalShortcuts);
    api.discord.setAppId(s.discordEnabled && s.discordAppId ? s.discordAppId : null);
  };
  pushSettings();
  useSettings.subscribe((s, prev) => {
    if (
      s.closeToTray !== prev.closeToTray ||
      s.minimizeToTray !== prev.minimizeToTray ||
      s.globalShortcuts !== prev.globalShortcuts ||
      s.discordEnabled !== prev.discordEnabled ||
      s.discordAppId !== prev.discordAppId
    ) {
      pushSettings();
      if (s.discordEnabled && !prev.discordEnabled) pushPresence(true);
    }
  });

  api.onPlayerCommand((action) => {
    if (action === "togglePlay") player.toggle();
    else if (action === "next") player.next();
    else if (action === "prev") player.prev();
  });

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
    if (!track || !useSettings.getState().discordEnabled) {
      api.discord.update(null);
      return;
    }
    const startedAt = Date.now() - s.positionMs;
    api.discord.update({
      title: track.title,
      artist: artistLine(track.artists),
      artwork: track.artwork,
      url: track.permalink,
      paused: !playing,
      startedAt,
      endsAt: s.durationMs ? startedAt + s.durationMs : undefined,
    });
  };

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
