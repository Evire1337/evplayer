import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { IconMute, IconVolume } from "./Icons";

export default function VolumePanel() {
  const { volume, muted } = usePlayer();
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
        aria-label="Громкость"
        style={{ "--fill": `${value}%` } as React.CSSProperties}
      />
      <output htmlFor="volume-vertical" className="volume-value">{value}%</output>
      <button type="button" className="step-btn" onClick={() => usePlayer.setState({ muted: !muted })} aria-label={muted ? "Включить звук" : "Выключить звук"}>
        {muted || value === 0 ? <IconMute size={16} /> : <IconVolume size={16} />}
      </button>
    </div>
  );
}
