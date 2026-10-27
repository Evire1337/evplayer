import { player } from "@/player/engine";
import { currentTrack, usePlayer } from "@/player/store";
import { useLibrary } from "@/library/store";
import { useNavigation } from "@/ui/navigation";

// keys inside the window (the settings page lists them)

function typing(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
}

export function startShortcuts(): void {
  window.addEventListener("keydown", (e) => {
    if (e.defaultPrevented) return;
    const ctrl = e.ctrlKey || e.metaKey;
    if (ctrl && e.key.toLowerCase() === "f") {
      e.preventDefault();
      useNavigation.getState().go("search");
      return;
    }
    if (ctrl && e.key === ",") {
      e.preventDefault();
      useNavigation.getState().go("settings");
      return;
    }
    if (typing(e.target) || e.altKey) return;
    const s = usePlayer.getState();
    switch (e.key) {
      case " ":
        if ((e.target as HTMLElement)?.tagName === "BUTTON") return;
        e.preventDefault();
        player.toggle();
        break;
      case "ArrowRight":
        e.preventDefault();
        if (ctrl) player.next();
        else player.seek(Math.min(s.durationMs, s.positionMs + 5000));
        break;
      case "ArrowLeft":
        e.preventDefault();
        if (ctrl) player.prev();
        else player.seek(Math.max(0, s.positionMs - 5000));
        break;
      case "ArrowUp":
        // a focused list or slider keeps its arrows
        if (document.activeElement && document.activeElement !== document.body) return;
        e.preventDefault();
        player.setVolume(s.volume + 0.05);
        break;
      case "ArrowDown":
        if (document.activeElement && document.activeElement !== document.body) return;
        e.preventDefault();
        player.setVolume(s.volume - 0.05);
        break;
      default: {
        if (ctrl) return;
        const key = e.key.toLowerCase();
        if (key === "m" || key === "ь") usePlayer.setState({ muted: !s.muted });
        else if (key === "l" || key === "д") {
          const track = currentTrack(s);
          if (track) useLibrary.getState().toggleLike(track);
        } else if (key === "s" || key === "ы") player.toggleShuffle();
        else if (key === "r" || key === "к") player.cycleRepeat();
      }
    }
  });
}
