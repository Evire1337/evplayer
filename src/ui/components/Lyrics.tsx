import { useEffect, useRef } from "react";
import type { Track } from "@/types";
import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { activeLineIndex } from "@/services/lyrics/lrc";
import { useLyrics } from "../useLyrics";

// lines light up as they are sung; the current one is kept in the middle; a click jumps there
export default function Lyrics({ track }: { track: Track | undefined }) {
  const lyrics = useLyrics(track);
  const boxRef = useRef<HTMLDivElement>(null);
  const synced = lyrics.status === "ready" && lyrics.synced;
  const lines = lyrics.status === "ready" ? lyrics.lines : null;
  // only the line number is watched, so the text re-renders once per line, not on every tick;
  // a quarter second early feels right: you see the line as the singer starts it
  const active = usePlayer((s) => (synced && lines ? activeLineIndex(lines, s.positionMs + 250) : -1));

  useEffect(() => {
    const box = boxRef.current;
    if (!box || active < 0) return;
    const el = box.querySelector<HTMLElement>(`[data-line="${active}"]`);
    if (!el) return;
    box.scrollTo({ top: el.offsetTop - box.clientHeight / 2 + el.clientHeight / 2, behavior: "smooth" });
  }, [active]);

  if (!track) return <div className="lyrics-empty">Включите трек, и здесь появится его текст.</div>;
  if (lyrics.status === "loading" || lyrics.status === "idle") return <div className="lyrics-empty">Ищем текст…</div>;
  if (lyrics.status !== "ready") return <div className="lyrics-empty">{lyrics.status === "missing" ? "Текста для этого трека пока нет." : "Не получилось загрузить текст."}</div>;

  return (
    <div className={`lyrics${synced ? " is-synced" : ""}`} ref={boxRef}>
      <div className="lyrics-pad" />
      {lyrics.lines.map((line, i) => {
        const distance = active < 0 ? 3 : Math.abs(i - active);
        const cls = !synced ? "" : i === active ? " is-active" : i < active ? " is-past" : distance === 1 ? " is-near" : distance > 8 ? " is-far" : "";
        return synced ? (
          <button key={i} type="button" data-line={i} className={`lyric-line${cls}`} onClick={() => player.seek(line.timeMs)}>
            {line.text || "♪"}
          </button>
        ) : (
          <p key={i} className="lyric-line is-plain">{line.text}</p>
        );
      })}
      <div className="lyrics-pad" />
      <span className="lyrics-source">Текст: {lyrics.source}</span>
    </div>
  );
}
