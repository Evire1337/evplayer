import type React from "react";
import Artwork from "@/ui/components/Artwork";
import type { Track } from "@/types";
import { player } from "@/player/engine";
import { currentTrack, usePlayer } from "@/player/store";
import { useIsLiked, useLibrary } from "@/library/store";
import { useSettings } from "@/settings/store";
import { trackMenuItems, type TrackMenuContext } from "@/library/trackMenu";
import { openMenu } from "../menu";
import { artistLine, formatCount, formatTime } from "../format";
import { formatAddedAt } from "../dates";
import { IconGrip, IconHeart, IconHeartFill, IconMore, IconPause, IconPlay } from "./Icons";

export interface TrackRowProps {
  track: Track;
  queue: Track[];
  index: number;
  addedAt?: string;
  menu?: TrackMenuContext;
  number?: number;
  /** drag to reorder: the row's handlers from useReorder */
  reorder?: { "data-reorder": number; onPointerDown: (e: React.PointerEvent) => void };
  dropHint?: "before" | "after" | null;
  /** a small SC / YT / file badge next to the artist */
  showSource?: boolean;
  /** text for the right column instead of the service play count ("12 прослушиваний", "FLAC") */
  note?: string;
}

const SOURCE_LABEL = { soundcloud: "SC", youtube: "YT", local: "Файл" } as const;

export default function TrackRow({ track, queue, index, addedAt, menu, number, reorder, dropHint, showSource, note }: TrackRowProps) {
  const draggable = Boolean(reorder);
  const isCurrent = usePlayer((s) => currentTrack(s)?.id === track.id);
  const playing = usePlayer((s) => s.status === "playing");
  const liked = useIsLiked(track.id);
  const active = isCurrent && playing;
  return (
    <div
      className={`track-row${isCurrent ? " is-current" : ""}${addedAt !== undefined ? " has-date" : ""}${dropHint ? ` drop-${dropHint}` : ""}`}
      role="button"
      tabIndex={0}
      onClick={() => {
        if (useSettings.getState().doubleClickToPlay) return;
        if (isCurrent) player.toggle();
        else void player.playQueue(queue, index);
      }}
      onDoubleClick={() => {
        if (!useSettings.getState().doubleClickToPlay) return;
        if (isCurrent) player.toggle();
        else void player.playQueue(queue, index);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") isCurrent ? player.toggle() : void player.playQueue(queue, index);
      }}
      onContextMenu={(e) => openMenu(e, trackMenuItems(track, menu))}
      {...reorder}
    >
      <span className="track-lead">
        {draggable ? (
          <span className="track-grip" aria-hidden="true"><IconGrip size={16} /></span>
        ) : number !== undefined ? (
          <span className="track-num">{number}</span>
        ) : null}
        <span className="track-art">
          <Artwork src={track.artwork} size="row" />
          <span className="track-art-overlay">{active ? <IconPause size={16} /> : <IconPlay size={16} />}</span>
        </span>
      </span>
      <span className="track-text">
        <span className="track-title">{track.title}</span>
        <span className="track-artist">
          {showSource && <span className={`source-badge is-${track.source}`}>{SOURCE_LABEL[track.source]}</span>}
          {artistLine(track.artists)}
        </span>
      </span>
      {addedAt !== undefined && <span className="track-date">{formatAddedAt(addedAt)}</span>}
      {note !== undefined && addedAt === undefined ? (
        <span className="track-plays">{note}</span>
      ) : (
        track.plays !== undefined && addedAt === undefined && <span className="track-plays" data-tip="Прослушивания на сервисе">{formatCount(track.plays)}</span>
      )}
      <button
        type="button"
        className={`row-icon${liked ? " is-liked" : ""}`}
        aria-label={liked ? "Убрать из понравившихся" : "Нравится"} data-tip={liked ? "Убрать из понравившихся" : "Нравится"}
        onClick={(e) => {
          e.stopPropagation();
          useLibrary.getState().toggleLike(track);
        }}
      >
        {liked ? <IconHeartFill size={17} /> : <IconHeart size={17} />}
      </button>
      <span className="track-time">{track.durationMs > 0 ? formatTime(track.durationMs) : ""}</span>
      <button
        type="button"
        className="row-icon"
        aria-label="Ещё" data-tip="Ещё"
        onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          openMenu({ clientX: r.left, clientY: r.bottom + 4, preventDefault: () => e.preventDefault(), stopPropagation: () => e.stopPropagation() }, trackMenuItems(track, menu));
        }}
      >
        <IconMore size={18} />
      </button>
    </div>
  );
}
