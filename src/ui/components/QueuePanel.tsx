import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { artistLine } from "../format";

const SOURCE_LABEL = { soundcloud: "SoundCloud", youtube: "YouTube Music", local: "Мои файлы" } as const;

export default function QueuePanel() {
  const { queue, index } = usePlayer();
  const upcoming = queue.slice(index + 1, index + 41);
  return (
    <aside className="queue glass" aria-label="Далее в очереди">
      <h2 className="queue-title">Далее</h2>
      {upcoming.length === 0 ? (
        <p className="muted small">Очередь пуста. Выберите трек на главной или в поиске.</p>
      ) : (
        <ol className="queue-list">
          {upcoming.map((t, i) => (
            <li key={`${t.id}-${i}`}>
              <button type="button" className="queue-item" onClick={() => void player.playIndex(index + 1 + i)}>
                <span className="queue-art">{t.artwork ? <img src={t.artwork} alt="" loading="lazy" /> : null}</span>
                <span className="queue-text">
                  <span className="queue-name">{t.title}</span>
                  <span className="queue-sub">{artistLine(t.artists)} · {SOURCE_LABEL[t.source]}</span>
                </span>
              </button>
            </li>
          ))}
        </ol>
      )}
    </aside>
  );
}
