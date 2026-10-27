import { useShallow } from "zustand/react/shallow";
import { player } from "@/player/engine";
import { currentTrack, usePlayer } from "@/player/store";
import { useEffects } from "@/player/effects";
import { formatTime } from "../format";
import { IconNext, IconPause, IconPlay, IconPrev, IconRepeat, IconShuffle, IconSliders, IconVolume, IconMute } from "./Icons";
import Popover from "./Popover";
import SoundPanel from "./SoundPanel";
import VolumePanel from "./VolumePanel";

export function Progress({ big = false }: { big?: boolean }) {
  const { positionMs, durationMs } = usePlayer(useShallow((s) => ({ positionMs: s.positionMs, durationMs: s.durationMs })));
  const track = usePlayer((s) => currentTrack(s));
  const progress = durationMs > 0 ? Math.min(1, positionMs / durationMs) : 0;
  return (
    <div className={`progress${big ? " is-big" : ""}`}>
      <input
        id={big ? "seek-big" : "seek-mini"}
        type="range"
        className="slider"
        min={0}
        max={1000}
        value={Math.round(progress * 1000)}
        disabled={!track}
        aria-label="Позиция"
        onChange={(e) => player.seek((Number(e.target.value) / 1000) * durationMs)}
        style={{ "--fill": `${progress * 100}%` } as React.CSSProperties}
      />
      <div className="progress-times">
        <span>{formatTime(positionMs)}</span>
        <span>−{formatTime(Math.max(0, durationMs - positionMs))}</span>
      </div>
    </div>
  );
}

export function Transport({ big = false }: { big?: boolean }) {
  const { status, shuffle, repeat } = usePlayer(useShallow((s) => ({ status: s.status, shuffle: s.shuffle, repeat: s.repeat })));
  const track = usePlayer((s) => currentTrack(s));
  const playing = status === "playing";
  return (
    <div className={`transport${big ? " is-big" : ""}`}>
      <button type="button" className={`round-btn ghost${shuffle ? " is-on" : ""}`} aria-label="Перемешать" aria-pressed={shuffle} onClick={() => player.toggleShuffle()}>
        <IconShuffle size={19} />
      </button>
      <button type="button" className="round-btn soft" aria-label="Предыдущий" disabled={!track} onClick={() => player.prev()}>
        <IconPrev size={big ? 22 : 18} />
      </button>
      <button type="button" className="round-btn main" aria-label={playing ? "Пауза" : "Играть"} disabled={!track} onClick={() => player.toggle()}>
        {status === "loading" ? <span className="spinner" /> : playing ? <IconPause size={big ? 26 : 20} /> : <IconPlay size={big ? 26 : 20} />}
      </button>
      <button type="button" className="round-btn soft" aria-label="Следующий" disabled={!track} onClick={() => player.next()}>
        <IconNext size={big ? 22 : 18} />
      </button>
      <button type="button" className={`round-btn ghost${repeat !== "off" ? " is-on" : ""}`} aria-label={`Повтор: ${repeat === "one" ? "трек" : repeat === "all" ? "очередь" : "выключен"}`} onClick={() => player.cycleRepeat()}>
        <IconRepeat size={19} />
        {repeat === "one" && <span className="repeat-one">1</span>}
      </button>
    </div>
  );
}

export function Tools() {
  const fx = useEffects();
  const tuned = fx.speed !== 1 || (!fx.linked && fx.pitch !== 0) || fx.reverb;
  const { volume, muted } = usePlayer(useShallow((s) => ({ volume: s.volume, muted: s.muted })));
  return (
    <div className="tools">
      <Popover label="Звук: скорость, тон, реверберация" icon={<IconSliders size={19} />} width={330} active={tuned}>
        <SoundPanel />
      </Popover>
      <Popover label="Громкость" icon={muted || volume === 0 ? <IconMute size={19} /> : <IconVolume size={19} />} width={76}>
        <VolumePanel />
      </Popover>
    </div>
  );
}
