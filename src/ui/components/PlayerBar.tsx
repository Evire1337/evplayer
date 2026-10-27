import { player } from "@/player/engine";
import { currentTrack, usePlayer } from "@/player/store";
import { artistLine, formatTime } from "../format";
import { IconMute, IconNext, IconPause, IconPlay, IconPrev, IconRepeat, IconShuffle, IconVolume } from "./Icons";

export default function PlayerBar() {
  const track = usePlayer((s) => currentTrack(s));
  const { status, positionMs, durationMs, volume, muted, shuffle, repeat, error } = usePlayer();
  const playing = status === "playing";
  const progress = durationMs > 0 ? Math.min(1, positionMs / durationMs) : 0;

  return (
    <footer className="player glass" aria-label="Плеер">
      <div className="player-now">
        <span className="player-art">{track?.artwork ? <img src={track.artwork} alt="" /> : null}</span>
        <span className="player-text">
          <span className="player-title">{track?.title ?? "Ничего не играет"}</span>
          <span className="player-artist">{error ? <span className="player-error">{error}</span> : track ? artistLine(track.artists) : "Найдите трек в поиске"}</span>
        </span>
      </div>

      <div className="player-center">
        <div className="player-controls">
          <button type="button" className={`icon-btn${shuffle ? " is-on" : ""}`} onClick={() => player.toggleShuffle()} aria-label="Перемешать" aria-pressed={shuffle}>
            <IconShuffle size={18} />
          </button>
          <button type="button" className="icon-btn" onClick={() => player.prev()} aria-label="Предыдущий" disabled={!track}>
            <IconPrev size={18} />
          </button>
          <button type="button" className="play-btn" onClick={() => player.toggle()} aria-label={playing ? "Пауза" : "Играть"} disabled={!track}>
            {status === "loading" ? <span className="spinner" /> : playing ? <IconPause size={20} /> : <IconPlay size={20} />}
          </button>
          <button type="button" className="icon-btn" onClick={() => player.next()} aria-label="Следующий" disabled={!track}>
            <IconNext size={18} />
          </button>
          <button type="button" className={`icon-btn${repeat !== "off" ? " is-on" : ""}`} onClick={() => player.cycleRepeat()} aria-label={`Повтор: ${repeat === "one" ? "трек" : repeat === "all" ? "очередь" : "выкл"}`}>
            <IconRepeat size={18} />
            {repeat === "one" && <span className="repeat-one">1</span>}
          </button>
        </div>
        <div className="player-progress">
          <span className="time">{formatTime(positionMs)}</span>
          <input
            id="seek"
            type="range"
            className="slider"
            min={0}
            max={1000}
            value={Math.round(progress * 1000)}
            onChange={(e) => player.seek((Number(e.target.value) / 1000) * durationMs)}
            aria-label="Позиция"
            disabled={!track}
            style={{ "--fill": `${progress * 100}%` } as React.CSSProperties}
          />
          <span className="time">{formatTime(durationMs)}</span>
        </div>
      </div>

      <div className="player-volume">
        <button type="button" className="icon-btn" onClick={() => usePlayer.setState({ muted: !muted })} aria-label={muted ? "Включить звук" : "Выключить звук"}>
          {muted || volume === 0 ? <IconMute size={18} /> : <IconVolume size={18} />}
        </button>
        <input
          id="volume"
          type="range"
          className="slider"
          min={0}
          max={100}
          value={muted ? 0 : Math.round(volume * 100)}
          onChange={(e) => player.setVolume(Number(e.target.value) / 100)}
          aria-label="Громкость"
          style={{ "--fill": `${(muted ? 0 : volume) * 100}%` } as React.CSSProperties}
        />
      </div>
    </footer>
  );
}
