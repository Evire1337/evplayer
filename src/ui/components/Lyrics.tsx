import { useSettings } from "@/settings/store";
import { useEffect, useRef } from "react";
import type { Track } from "@/types";
import { player } from "@/player/engine";
import { useLyrics } from "../useLyrics";
import { OFFSET_STEP_MS, useActiveLine, useLyricsOffset, useLyricsOffsets } from "../lyricsTiming";

function seconds(ms: number): string {
  const v = (ms / 1000).toFixed(2).replace(/0$/, "").replace(".", ",");
  return `${ms > 0 ? "+" : ms < 0 ? "−" : ""}${v.replace("-", "")} с`;
}

// lines light up as they are sung; the current one is kept in the middle; a click jumps there
export default function Lyrics({ track }: { track: Track | undefined }) {
  const lyrics = useLyrics(track);
  const boxRef = useRef<HTMLDivElement>(null);
  const synced = lyrics.status === "ready" && lyrics.synced;
  const lines = lyrics.status === "ready" ? lyrics.lines : null;
  // only the line number is watched, so the text re-renders once per line, not on every tick
  const active = useActiveLine(synced ? lines : null, track?.id);
  const offset = useLyricsOffset(track?.id);

  useEffect(() => {
    const box = boxRef.current;
    if (!box || active < 0) return;
    const el = box.querySelector<HTMLElement>(`[data-line="${active}"]`);
    if (!el) return;
    box.scrollTo({ top: el.offsetTop - box.clientHeight / 2 + el.clientHeight / 2, behavior: useSettings.getState().memorySaver ? "auto" : "smooth" });
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
          <button key={i} type="button" data-line={i} className={`lyric-line${cls}`} onClick={() => player.seek(Math.max(0, line.timeMs - offset))}>
            {line.text || "♪"}
          </button>
        ) : (
          <p key={i} className="lyric-line is-plain">{line.text}</p>
        );
      })}
      <div className="lyrics-pad" />
      <span className="lyrics-source">Текст: {lyrics.source}</span>
      {synced && track && (
        // the text can be timed for a slightly different version of the song: nudge it
        <div className={`lyrics-shift glass${offset ? " is-set" : ""}`}>
          <button type="button" className="lyrics-shift-btn" data-tip="Текст позже" aria-label="Показывать текст позже" onClick={() => useLyricsOffsets.getState().nudge(track.id, -OFFSET_STEP_MS)}>−</button>
          <button type="button" className="lyrics-shift-value" data-tip={offset ? "Сбросить сдвиг" : "Текст отстаёт или спешит? Сдвиньте его"} onClick={() => useLyricsOffsets.getState().reset(track.id)}>
            {offset ? `сдвиг ${seconds(offset)}` : "синхронно"}
          </button>
          <button type="button" className="lyrics-shift-btn" data-tip="Текст раньше" aria-label="Показывать текст раньше" onClick={() => useLyricsOffsets.getState().nudge(track.id, OFFSET_STEP_MS)}>+</button>
        </div>
      )}
    </div>
  );
}
