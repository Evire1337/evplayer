import type { Playlist } from "./store";

/** a playlist's picture: your own icon, else a mosaic of its first covers */
export default function PlaylistCover({ playlist, liked = false, size }: { playlist?: Playlist; liked?: boolean; size?: number }) {
  const style = size ? { width: size, height: size } : undefined;
  if (liked) {
    return (
      <span className="pl-cover is-liked" style={style} aria-hidden="true">
        <svg width="42%" height="42%" viewBox="0 0 24 24" fill="currentColor"><path d="M12 20s-7-4.4-7-10a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.6-7 10-7 10z" /></svg>
      </span>
    );
  }
  if (playlist?.icon) {
    return (
      <span className="pl-cover" style={style}>
        <img src={playlist.icon} alt="" />
      </span>
    );
  }
  const arts = [...new Set((playlist?.items ?? []).map((i) => i.track.artwork).filter(Boolean))].slice(0, 4) as string[];
  if (arts.length === 0) {
    return (
      <span className="pl-cover is-empty" style={style} aria-hidden="true">
        <svg width="36%" height="36%" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6"><path d="M9 18V6l10-2v12" /><circle cx="6.5" cy="18" r="2.5" /><circle cx="16.5" cy="16" r="2.5" /></svg>
      </span>
    );
  }
  if (arts.length < 4) {
    return (
      <span className="pl-cover" style={style}>
        <img src={arts[0]} alt="" />
      </span>
    );
  }
  return (
    <span className="pl-cover is-grid" style={style}>
      {arts.map((a) => (
        <img key={a} src={a} alt="" />
      ))}
    </span>
  );
}
