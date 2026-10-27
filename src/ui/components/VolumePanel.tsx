import { useShallow } from "zustand/react/shallow";
import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";

export default function VolumePanel() {
  const { volume, muted } = usePlayer(useShallow((s) => ({ volume: s.volume, muted: s.muted })));
  const value = muted ? 0 : Math.round(volume * 100);
  return (
    <div className="volume-panel">
      <input
        id="volume-vertical"
        type="range"
        className="vslider"
        min={0}
        max={100}
        value={value}
        onChange={(e) => player.setVolume(Number(e.target.value) / 100)}
        onWheel={(e) => player.setVolume((value + (e.deltaY < 0 ? 5 : -5)) / 100)}
        aria-label="Громкость"
        style={{ "--fill": `${value}%` } as React.CSSProperties}
      />
      <output htmlFor="volume-vertical" className={`volume-value${value > 82 ? " is-on-fill" : ""}`}>{value}</output>
    </div>
  );
}
