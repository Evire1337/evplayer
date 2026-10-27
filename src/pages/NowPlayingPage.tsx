import Artwork from "@/ui/components/Artwork";
import { currentTrack, usePlayer } from "@/player/store";
import Lyrics from "@/ui/components/Lyrics";
import QueuePanel from "@/ui/components/QueuePanel";
import { Progress, Tools, Transport } from "@/ui/components/Transport";
import { useNavigation } from "@/ui/navigation";
import { useSettings } from "@/settings/store";
import { IconQueue } from "@/ui/components/Icons";

const SOURCE = { soundcloud: "SoundCloud", youtube: "YouTube Music", local: "Мои файлы" } as const;

export default function NowPlayingPage() {
  const track = usePlayer((s) => currentTrack(s));
  const error = usePlayer((s) => s.error);
  const route = usePlayer((s) => s.route);
  const go = useNavigation((s) => s.go);
  const showQueue = useSettings((s) => s.showQueue);
  return (
    <div className={`now${showQueue ? "" : " no-queue"}`}>
      <section className="now-left" aria-label="Обложка и управление">
        <div className="now-art"><Artwork src={track?.artwork} lazy={false} /></div>
        <div className="now-meta">
          <h1 className="now-title">{track?.title ?? "Ничего не играет"}</h1>
          <span className="now-artist">
            {error ? (
              <span className="error-text">{error}</span>
            ) : track ? (
              track.artists.map((a, i) => (
                <span key={a.id + i}>
                  {i > 0 && ", "}
                  <button type="button" className="link-btn" onClick={() => go("artist", a.id, a.name)}>{a.name}</button>
                </span>
              ))
            ) : (
              "Выберите трек на главной или в поиске"
            )}
          </span>
          {track && (
            <span className="now-source">
              {SOURCE[track.source]}
              {route === "matched" && " · играет копия с SoundCloud"}
            </span>
          )}
        </div>
        <Progress big />
        <Transport big />
        <div className="now-tools">
          <Tools />
          <button
            type="button"
            className={`tool-btn${showQueue ? " is-on" : ""}`}
            aria-label={showQueue ? "Скрыть «Далее»" : "Показать «Далее»"}
            data-tip={showQueue ? "Скрыть «Далее»" : "Показать «Далее»"}
            aria-pressed={showQueue}
            onClick={() => useSettings.getState().set({ showQueue: !showQueue })}
          >
            <IconQueue size={19} />
          </button>
        </div>
      </section>
      <section className="now-lyrics" aria-label="Текст песни">
        <Lyrics track={track} />
      </section>
      {showQueue && <QueuePanel />}
    </div>
  );
}
