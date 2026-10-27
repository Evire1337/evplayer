import { useState } from "react";
import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { useSettings } from "@/settings/store";
import { trackMenuItems } from "@/library/trackMenu";
import { openMenu } from "../menu";
import { artistLine } from "../format";

const SOURCE_LABEL = { soundcloud: "SoundCloud", youtube: "YouTube Music", local: "Мои файлы" } as const;

export default function QueuePanel() {
  const queue = usePlayer((s) => s.queue);
  const index = usePlayer((s) => s.index);
  const autoplay = useSettings((s) => s.autoplay);
  const [drag, setDrag] = useState<{ from: number; over: number } | null>(null);
  const upcoming = queue.slice(index + 1, index + 61);
  return (
    <aside className="queue glass" aria-label="Далее в очереди">
      <div className="queue-head">
        <h2 className="queue-title">Далее</h2>
        <span className="queue-actions">
          {upcoming.length > 0 && (
            <button type="button" className="reset-btn" onClick={() => player.clearUpcoming()}>Очистить</button>
          )}
          <button type="button" className="reset-btn" onClick={() => useSettings.getState().set({ showQueue: false })}>Скрыть</button>
        </span>
      </div>
      {upcoming.length === 0 ? (
        <p className="muted small">{autoplay ? "Очередь пуста — после этого трека включатся похожие." : "Очередь пуста. Выберите трек на главной или в поиске."}</p>
      ) : (
        <ol className="queue-list">
          {upcoming.map((t, i) => {
            const at = index + 1 + i;
            return (
              <li
                key={`${t.id}-${at}`}
                draggable
                className={drag && drag.over === at && drag.from !== at ? (drag.from < at ? "drop-after" : "drop-before") : ""}
                onDragStart={(e) => {
                  e.dataTransfer.effectAllowed = "move";
                  setDrag({ from: at, over: at });
                }}
                onDragOver={(e) => {
                  if (!drag) return;
                  e.preventDefault();
                  if (drag.over !== at) setDrag({ ...drag, over: at });
                }}
                onDrop={(e) => {
                  e.preventDefault();
                  if (drag) player.moveInQueue(drag.from, at);
                  setDrag(null);
                }}
                onDragEnd={() => setDrag(null)}
              >
                <button
                  type="button"
                  className="queue-item"
                  onClick={() => void player.playIndex(at)}
                  onContextMenu={(e) => openMenu(e, trackMenuItems(t, { queueIndex: at }))}
                >
                  <span className="queue-art">{t.artwork ? <img src={t.artwork} alt="" loading="lazy" draggable={false} /> : null}</span>
                  <span className="queue-text">
                    <span className="queue-name">{t.title}</span>
                    <span className="queue-sub">{artistLine(t.artists)} · {SOURCE_LABEL[t.source]}</span>
                  </span>
                </button>
              </li>
            );
          })}
        </ol>
      )}
    </aside>
  );
}
