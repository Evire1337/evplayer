import { player } from "@/player/engine";
import { currentTrack, usePlayer } from "@/player/store";
import { useLibrary } from "@/library/store";
import { useNavigation } from "@/ui/navigation";
import type { ShortcutAction } from "@/settings/shortcuts";

// One place that knows what each command does: keys in the window, system-wide keys,
// the tray menu and the mini player all end up here.

export type Command = ShortcutAction | "togglePlay" | "seekTo" | "prevTrack";

export function runAction(action: Command, value?: number): void {
  const s = usePlayer.getState();
  switch (action) {
    case "playPause":
    case "togglePlay":
      return player.toggle();
    case "next":
      return player.next();
    case "prev":
      return player.prev();
    case "prevTrack":
      return player.prev(true);
    case "seekForward":
      return player.seek(Math.min(s.durationMs, s.positionMs + 5000));
    case "seekBackward":
      return player.seek(Math.max(0, s.positionMs - 5000));
    case "seekTo":
      if (typeof value === "number") player.seek(value);
      return;
    case "volumeUp":
      return player.setVolume(s.volume + 0.05);
    case "volumeDown":
      return player.setVolume(s.volume - 0.05);
    case "mute":
      return usePlayer.setState({ muted: !s.muted });
    case "like": {
      const track = currentTrack(s);
      if (track) useLibrary.getState().toggleLike(track);
      return;
    }
    case "shuffle":
      return player.toggleShuffle();
    case "repeat":
      return player.cycleRepeat();
    case "search":
      return useNavigation.getState().go("search");
    case "settings":
      return useNavigation.getState().go("settings");
    case "nowPlaying":
      return useNavigation.getState().go("now");
    case "toggleOverlay":
      return window.evplayer?.overlay.toggle();
    case "hideOverlay":
      return window.evplayer?.overlay.hide();
    case "showWindow":
      return window.evplayer?.showWindow();
  }
}
