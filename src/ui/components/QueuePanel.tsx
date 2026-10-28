import Artwork from "@/ui/components/Artwork";
import { useLayoutEffect, useRef } from "react";
import { useReorder } from "../useReorder";
import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { useSettings } from "@/settings/store";
import { trackMenuItems } from "@/library/trackMenu";
import { openMenu } from "../menu";
import { artistLine } from "../format";

/** how many already played songs stay visible above the current one */
const PLAYED_SHOWN = 30;

const SOURCE_LABEL = { soundcloud: "SoundCloud", youtube: "YouTube Music", local: "Мои файлы" } as const;

export default function QueuePanel({ onHide }: { onHide?: () => void } = {}) {
  const queue = usePlayer((s) => s.queue);
  const index = usePlayer((s) => s.index);
  const autoplay = useSettings((s) => s.autoplay);
  // positions here are queue positions, so a move is a move in the queue
  const reorder = useReorder((from, to) => player.moveInQueue(from, to));
  const upcoming = queue.slice(index + 1, index + 61);
  // what already played stays above, dimmed; the list opens at the current song
  const firstShown = Math.max(0, index - PLAYED_SHOWN);
  const shown = queue.slice(firstShown, index + 61);
  const listRef = useRef<HTMLOListElement>(null);
  const currentId = queue[index]?.id;
  useLayoutEffect(() => {
    const list = listRef.current;
    const current = list?.querySelector<HTMLElement>("li.is-current");
    if (list && current) list.scrollTop = current.offsetTop - list.offsetTop;
  }, [index, currentId]);
  return (
    <aside className="queue glass" aria-label="Далее в очереди">
      <div className="queue-head">
        <h2 className="queue-title">Далее</h2>
        <span className="queue-actions">
          {upcoming.length > 0 && (
            <button type="button" className="reset-btn" onClick={() => player.clearUpcoming()}>Очистить</button>
          )}
          <button type="button" className="reset-btn" onClick={() => (onHide ? onHide() : useSettings.getState().set({ showQueue: false }))}>Скрыть</button>
        </span>
      </div>
      {shown.length <= 1 && upcoming.length === 0 ? (
        <p className="muted small">{autoplay ? "Очередь пуста — после этого трека включатся похожие." : "Очередь пуста. Выберите трек на главной или в поиске."}</p>
      ) : (
        <ol className="queue-list" ref={listRef}>
          {shown.map((t, i) => {
            const at = firstShown + i;
            const state = at < index ? " is-played" : at === index ? " is-current" : "";
            return (
              <li
                key={`${t.id}-${at}`}
                className={`${reorder.hint(at) ? `drop-${reorder.hint(at)}` : ""}${state}`}
                {...reorder.rowProps(at)}
              >
                <button
                  type="button"
                  className="queue-item"
                  onClick={() => (at === index ? player.toggle() : void player.playIndex(at))}
                  onContextMenu={(e) => openMenu(e, trackMenuItems(t, { queueIndex: at }))}
                >
                  <span className="queue-art"><Artwork src={t.artwork} size="row" /></span>
                  <span className="queue-text">
                    <span className="queue-name">{t.title}</span>
                    <span className="queue-sub">{artistLine(t.artists)} · {SOURCE_LABEL[t.source]}</span>
                  </span>
                </button>
              </li>
            );
          })}
          {upcoming.length === 0 && <li className="queue-note muted small">{autoplay ? "Дальше включатся похожие треки." : "Дальше ничего нет."}</li>}
        </ol>
      )}
    </aside>
  );
}
