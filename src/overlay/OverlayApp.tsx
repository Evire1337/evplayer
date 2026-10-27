import Artwork from "@/ui/components/Artwork";
import { useEffect, useRef, useState } from "react";
import { IconExpand, IconHeart, IconHeartFill, IconNext, IconPause, IconPlay, IconPrev } from "@/ui/components/Icons";
import { formatTime } from "@/ui/format";
import { acceleratorFromEvent, actionFor } from "@/settings/shortcuts";
import { useSettings } from "@/settings/store";

// The mini player window. It has no player of its own: it shows what the main window
// sends and passes button presses back.

interface OverlayState {
  track: { id: string; title: string; artist: string; artwork?: string } | null;
  status: string;
  positionMs: number;
  durationMs: number;
  liked: boolean;
  line: string | null;
}

const api = () => window.evplayer?.overlay;

export default function OverlayApp() {
  const [state, setState] = useState<OverlayState | null>(null);
  const [pos, setPos] = useState(0);
  const received = useRef({ at: 0, pos: 0 });

  useEffect(() => {
    const off = api()?.onState((s) => {
      const next = s as OverlayState;
      received.current = { at: performance.now(), pos: next.positionMs };
      setState(next);
      setPos(next.positionMs);
    });
    api()?.ready();
    return off;
  }, []);

  // keys while the mini player has focus (Settings → Keys → Mini player)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const acc = acceleratorFromEvent(e);
      if (!acc) return;
      const action = actionFor(useSettings.getState().shortcuts, "overlay", acc);
      if (!action) return;
      e.preventDefault();
      if (action === "hideOverlay") api()?.hide();
      else if (action === "prev") api()?.command("prevTrack");
      else api()?.command(action);
    };
    // the main window changes the settings: pick them up
    const onStorage = (e: StorageEvent) => {
      if (e.key === "evplayer.settings.v1") void useSettings.persist.rehydrate();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("storage", onStorage);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("storage", onStorage);
    };
  }, []);

  // between updates the bar moves on by itself, so it stays smooth with few messages
  useEffect(() => {
    if (state?.status !== "playing") return;
    const timer = setInterval(() => setPos(received.current.pos + performance.now() - received.current.at), 250);
    return () => clearInterval(timer);
  }, [state?.status]);

  const cmd = (action: string, value?: number) => api()?.command(action, value);
  const track = state?.track;
  const playing = state?.status === "playing";
  const progress = state?.durationMs ? Math.min(1, pos / state.durationMs) : 0;

  return (
    <div className="overlay">
      <div className="overlay-card">
        <button type="button" className="overlay-art" onClick={() => cmd("showWindow")} aria-label="Открыть EvPlayer">
          <Artwork src={track?.artwork} lazy={false} size="row" />
          <span className="overlay-art-hover"><IconExpand size={13} /></span>
        </button>
        <div className="overlay-text">
          <span className="overlay-title">{track?.title ?? "Ничего не играет"}</span>
          <span className="overlay-sub">{state?.line ?? track?.artist ?? "Включите трек в EvPlayer"}</span>
        </div>
        <div className="overlay-controls">
          <button type="button" className={`overlay-btn${state?.liked ? " is-liked" : ""}`} aria-label="Нравится" onClick={() => cmd("like")} disabled={!track}>
            {state?.liked ? <IconHeartFill size={15} /> : <IconHeart size={15} />}
          </button>
          <button type="button" className="overlay-btn" aria-label="Предыдущий" onClick={() => cmd("prevTrack")} disabled={!track}>
            <IconPrev size={15} />
          </button>
          <button type="button" className="overlay-btn is-main" aria-label={playing ? "Пауза" : "Играть"} onClick={() => cmd("togglePlay")} disabled={!track}>
            {playing ? <IconPause size={15} /> : <IconPlay size={15} />}
          </button>
          <button type="button" className="overlay-btn" aria-label="Следующий" onClick={() => cmd("next")} disabled={!track}>
            <IconNext size={15} />
          </button>
        </div>
        <div
          className="overlay-progress"
          title={track ? `${formatTime(pos)} / ${formatTime(state?.durationMs ?? 0)}` : undefined}
          onClick={(e) => {
            if (!state?.durationMs) return;
            const r = e.currentTarget.getBoundingClientRect();
            cmd("seekTo", ((e.clientX - r.left) / r.width) * state.durationMs);
          }}
        >
          <span style={{ width: `${progress * 100}%` }} />
        </div>
      </div>
    </div>
  );
}
