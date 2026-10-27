import { currentTrack, usePlayer } from "@/player/store";
import Lyrics from "@/ui/components/Lyrics";
import QueuePanel from "@/ui/components/QueuePanel";
import { Progress, Tools, Transport } from "@/ui/components/Transport";
import { useNavigation } from "@/ui/navigation";

const SOURCE = { soundcloud: "SoundCloud", youtube: "YouTube Music", local: "Мои файлы" } as const;

export default function NowPlayingPage() {
  const track = usePlayer((s) => currentTrack(s));
  const error = usePlayer((s) => s.error);
  const route = usePlayer((s) => s.route);
  const go = useNavigation((s) => s.go);
  return (
    <div className="now">
      <section className="now-left" aria-label="Обложка и управление">
        <div className={`now-art${route === "embed" ? " is-embed" : ""}`}>{track?.artwork ? <img src={track.artwork} alt="" /> : null}</div>
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
              {route === "embed" && " · официальный плеер YouTube (эквалайзер и эффекты недоступны)"}
            </span>
          )}
        </div>
        <Progress big />
        <Transport big />
        <Tools />
      </section>
      <section className="now-lyrics" aria-label="Текст песни">
        <Lyrics track={track} />
      </section>
      <QueuePanel />
    </div>
  );
}
