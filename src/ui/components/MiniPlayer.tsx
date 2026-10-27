import { currentTrack, usePlayer } from "@/player/store";
import { useNavigation } from "../navigation";
import { artistLine } from "../format";
import { IconExpand } from "./Icons";
import { Progress, Tools, Transport } from "./Transport";

// the compact player that floats over the browsing pages
export default function MiniPlayer() {
  const track = usePlayer((s) => currentTrack(s));
  const error = usePlayer((s) => s.error);
  const go = useNavigation((s) => s.go);
  return (
    <footer className="mini glass" aria-label="Плеер">
      <button type="button" className="mini-now" onClick={() => go("now")} aria-label="Открыть «Сейчас играет»" disabled={!track}>
        <span className="mini-art">{track?.artwork ? <img src={track.artwork} alt="" /> : null}</span>
        <span className="mini-text">
          <span className="mini-title">{track?.title ?? "Ничего не играет"}</span>
          <span className="mini-sub">{error ? <span className="error-text">{error}</span> : track ? artistLine(track.artists) : "Найдите трек в поиске"}</span>
        </span>
      </button>
      <div className="mini-center">
        <Transport />
        <Progress />
      </div>
      <div className="mini-side">
        <Tools />
        <button type="button" className="tool-btn" onClick={() => go("now")} aria-label="Во весь экран" title="Во весь экран" disabled={!track}>
          <IconExpand size={18} />
        </button>
      </div>
    </footer>
  );
}
