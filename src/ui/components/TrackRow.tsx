import type { Track } from "@/types";
import { player } from "@/player/engine";
import { currentTrack, usePlayer } from "@/player/store";
import { artistLine, formatCount, formatTime } from "../format";
import { IconPause, IconPlay } from "./Icons";

export default function TrackRow({ track, queue, index }: { track: Track; queue: Track[]; index: number }) {
  const isCurrent = usePlayer((s) => currentTrack(s)?.id === track.id);
  const playing = usePlayer((s) => s.status === "playing");
  const active = isCurrent && playing;
  return (
    <button
      type="button"
      className={`track-row${isCurrent ? " is-current" : ""}`}
      onClick={() => (isCurrent ? player.toggle() : void player.playQueue(queue, index))}
    >
      <span className="track-art">
        {track.artwork ? <img src={track.artwork} alt="" loading="lazy" /> : null}
        <span className="track-art-overlay">{active ? <IconPause size={16} /> : <IconPlay size={16} />}</span>
      </span>
      <span className="track-text">
        <span className="track-title">{track.title}</span>
        <span className="track-artist">{artistLine(track.artists)}</span>
      </span>
      {track.plays !== undefined && <span className="track-plays">{formatCount(track.plays)}</span>}
      <span className="track-time">{formatTime(track.durationMs)}</span>
    </button>
  );
}
