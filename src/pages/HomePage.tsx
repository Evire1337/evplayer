import { useEffect, useState } from "react";
import type { Track } from "@/types";
import { soundCloudCharts } from "@/services/soundcloud/api";
import TrackCard from "@/ui/components/TrackCard";

function greeting(date = new Date()): string {
  const h = date.getHours();
  if (h < 5) return "Доброй ночи";
  if (h < 12) return "Доброе утро";
  if (h < 18) return "Добрый день";
  return "Добрый вечер";
}

function useChart(kind: "top" | "trending") {
  const [tracks, setTracks] = useState<Track[]>([]);
  const [state, setState] = useState<"loading" | "ready" | "error">("loading");
  useEffect(() => {
    const controller = new AbortController();
    soundCloudCharts(kind, controller.signal)
      .then((list) => {
        setTracks(list);
        setState("ready");
      })
      .catch(() => !controller.signal.aborted && setState("error"));
    return () => controller.abort();
  }, [kind]);
  return { tracks, state };
}

function Row({ title, tracks, state, ranked }: { title: string; tracks: Track[]; state: string; ranked?: boolean }) {
  return (
    <section className="row">
      <h2 className="row-title">{title}</h2>
      {state === "error" ? (
        <p className="muted">SoundCloud сейчас не отвечает. Попробуйте позже.</p>
      ) : (
        <div className="row-scroll">
          {state === "loading"
            ? Array.from({ length: 8 }, (_, i) => <span key={i} className="track-card skeleton" />)
            : tracks.map((t, i) => <TrackCard key={t.id} track={t} queue={tracks} index={i} rank={ranked ? i + 1 : undefined} />)}
        </div>
      )}
    </section>
  );
}

export default function HomePage() {
  const trending = useChart("trending");
  const top = useChart("top");
  return (
    <div className="page">
      <header className="page-head">
        <h1 className="display">{greeting()}</h1>
      </header>
      <Row title="В тренде на SoundCloud" {...trending} />
      <Row title="Топ SoundCloud" {...top} ranked />
    </div>
  );
}
