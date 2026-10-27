import Artwork from "@/ui/components/Artwork";
import type { Track } from "@/types";
import { player } from "@/player/engine";
import { currentTrack, usePlayer } from "@/player/store";
import { artistLine } from "../format";
import { IconPause, IconPlay } from "./Icons";
import { openMenu } from "../menu";
import { trackMenuItems } from "@/library/trackMenu";

export default function TrackCard({ track, queue, index, rank, sub }: { track: Track; queue: Track[]; index: number; rank?: number; sub?: string }) {
  // only the card of the song that is on re-renders when the player changes
  const isCurrent = usePlayer((s) => currentTrack(s)?.id === track.id);
  const playing = usePlayer((s) => isCurrent && s.status === "playing");
  const loading = usePlayer((s) => isCurrent && s.status === "loading");
  return (
    <button
      type="button"
      className={`track-card${isCurrent ? " is-current" : ""}`}
      onClick={() => (isCurrent ? player.toggle() : void player.playQueue(queue, index))}
      onContextMenu={(e) => openMenu(e, trackMenuItems(track))}
    >
      <span className="track-card-art">
        <Artwork src={track.artwork} />
        {rank !== undefined && <span className="track-card-rank">{rank}</span>}
        <span className="track-card-play" aria-label={playing ? "Пауза" : "Играть"}>
          {loading ? <span className="spinner" /> : playing ? <IconPause size={18} /> : <IconPlay size={18} />}
        </span>
        {playing && (
          <span className="eq-bars" aria-hidden="true">
            <i /><i /><i />
          </span>
        )}
      </span>
      <span className="track-card-title">{track.title}</span>
      <span className="track-card-artist">{sub ?? artistLine(track.artists)}</span>
    </button>
  );
}
