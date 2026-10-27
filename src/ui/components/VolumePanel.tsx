import { useShallow } from "zustand/react/shallow";
import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { IconMute, IconVolume } from "./Icons";

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
      <output htmlFor="volume-vertical" className="volume-value">{value}%</output>
      <button type="button" className="volume-mute" onClick={() => usePlayer.setState({ muted: !muted })} aria-label={muted ? "Включить звук" : "Выключить звук"}>
        {muted || value === 0 ? <IconMute size={14} /> : <IconVolume size={14} />}
      </button>
    </div>
  );
}
