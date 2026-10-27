import type { Track } from "@/types";
import { player } from "@/player/engine";
import { artistLine } from "../format";
import { IconPlay } from "./Icons";

export default function TrackCard({ track, queue, index, rank }: { track: Track; queue: Track[]; index: number; rank?: number }) {
  return (
    <button type="button" className="track-card" onClick={() => void player.playQueue(queue, index)}>
      <span className="track-card-art">
        {track.artwork ? <img src={track.artwork} alt="" loading="lazy" /> : null}
        {rank !== undefined && <span className="track-card-rank">{rank}</span>}
        <span className="track-card-play"><IconPlay size={18} /></span>
      </span>
      <span className="track-card-title">{track.title}</span>
      <span className="track-card-artist">{artistLine(track.artists)}</span>
    </button>
  );
}
