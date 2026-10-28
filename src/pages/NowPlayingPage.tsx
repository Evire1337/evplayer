import Artwork from "@/ui/components/Artwork";
import { currentTrack, usePlayer } from "@/player/store";
import Lyrics from "@/ui/components/Lyrics";
import QueuePanel from "@/ui/components/QueuePanel";
import { Progress, Tools, Transport } from "@/ui/components/Transport";
import { useNavigation } from "@/ui/navigation";
import { useSettings } from "@/settings/store";
import { IconQueue } from "@/ui/components/Icons";
import LikeButton from "@/ui/components/LikeButton";
import { useEffect, useState } from "react";
import { isPhone } from "@/platform/device";

/** the window is too narrow for the «Далее» column (the same width as in app.css) */
const NARROW = "(max-width: 1180px)";

function useNarrow(): boolean {
  const [narrow, setNarrow] = useState(() => typeof matchMedia !== "undefined" && matchMedia(NARROW).matches);
  useEffect(() => {
    const query = matchMedia(NARROW);
    const on = () => setNarrow(query.matches);
    query.addEventListener("change", on);
    return () => query.removeEventListener("change", on);
  }, []);
  return narrow;
}

const SOURCE = { soundcloud: "SoundCloud", youtube: "YouTube Music", local: "Мои файлы" } as const;

export default function NowPlayingPage() {
  const track = usePlayer((s) => currentTrack(s));
  const error = usePlayer((s) => s.error);
  const route = usePlayer((s) => s.route);
  const go = useNavigation((s) => s.go);
  const showSetting = useSettings((s) => s.showQueue);
  // a phone (or a narrow window) has no room for a third column: «Далее» slides up over
  // the page when asked
  const narrow = useNarrow();
  const asSheet = isPhone || narrow;
  const [sheet, setSheet] = useState(false);
  const showQueue = asSheet ? sheet : showSetting;
  const toggleQueue = () => (asSheet ? setSheet(!sheet) : useSettings.getState().set({ showQueue: !showSetting }));
  return (
    <div className={`now${showQueue && !asSheet ? "" : " no-queue"}`}>
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
              {route === "soundcloud" && " · играет копия с SoundCloud"}
              {route === "youtube" && " · играет копия с YouTube"}
            </span>
          )}
        </div>
        <Progress big />
        <Transport big />
        <div className="now-tools">
          <LikeButton track={track} size={19} />
          <Tools />
          <button
            type="button"
            className={`tool-btn${showQueue ? " is-on" : ""}`}
            aria-label={showQueue ? "Скрыть «Далее»" : "Показать «Далее»"}
            data-tip={showQueue ? "Скрыть «Далее»" : "Показать «Далее»"}
            aria-pressed={showQueue}
            onClick={toggleQueue}
          >
            <IconQueue size={19} />
          </button>
        </div>
      </section>
      <section className="now-lyrics" aria-label="Текст песни">
        <Lyrics track={track} />
      </section>
      {!asSheet && showQueue && <QueuePanel />}
      {asSheet && sheet && (
        <div className="queue-sheet" onClick={() => setSheet(false)}>
          <div className="queue-sheet-body" onClick={(e) => e.stopPropagation()}>
            <QueuePanel onHide={() => setSheet(false)} />
          </div>
        </div>
      )}
    </div>
  );
}
