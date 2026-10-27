import type { Track } from "@/types";
import { useIsLiked, useLibrary } from "@/library/store";
import { IconHeart, IconHeartFill } from "./Icons";

/** heart for the song that is playing: adds it to (or takes it out of) "Liked" */
export default function LikeButton({ track, size = 18, className = "tool-btn" }: { track: Track | null | undefined; size?: number; className?: string }) {
  const liked = useIsLiked(track?.id ?? "");
  const label = liked ? "Убрать из понравившихся" : "Нравится";
  return (
    <button
      type="button"
      className={`${className} like-btn${liked ? " is-liked" : ""}`}
      aria-label={label}
      data-tip={label}
      aria-pressed={liked}
      disabled={!track}
      onClick={() => track && useLibrary.getState().toggleLike(track)}
    >
      {liked ? <IconHeartFill size={size} /> : <IconHeart size={size} />}
    </button>
  );
}
