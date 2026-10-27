import { currentTrack, usePlayer } from "@/player/store";
import Lyrics from "@/ui/components/Lyrics";
import QueuePanel from "@/ui/components/QueuePanel";
import { Progress, Tools, Transport } from "@/ui/components/Transport";
import { artistLine } from "@/ui/format";

export default function NowPlayingPage() {
  const track = usePlayer((s) => currentTrack(s));
  const error = usePlayer((s) => s.error);
  return (
    <div className="now">
      <section className="now-left" aria-label="Обложка и управление">
        <div className="now-art">{track?.artwork ? <img src={track.artwork} alt="" /> : null}</div>
        <div className="now-meta">
          <h1 className="now-title">{track?.title ?? "Ничего не играет"}</h1>
          <span className="now-artist">{error ? <span className="error-text">{error}</span> : track ? artistLine(track.artists) : "Выберите трек на главной или в поиске"}</span>
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
