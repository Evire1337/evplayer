import { useEffect, useMemo, useState } from "react";
import type { Track } from "@/types";
import { soundCloudCharts } from "@/services/soundcloud/api";
import TrackCard from "@/ui/components/TrackCard";
import TrackRow from "@/ui/components/TrackRow";
import { Segmented } from "@/ui/components/Controls";
import { IconSparkle, IconUser } from "@/ui/components/Icons";
import { useHistory } from "@/stats/history";
import { getSmartPlaylist, getTopArtists, getTopTracks, listenedToday, SMART_PLAYLISTS, type StatsRange } from "@/stats/listeningStats";
import { useStatsVersion } from "@/stats/useStats";
import { SMART_INFO } from "@/stats/smart";
import { formatListenTime, playsWord } from "@/stats/format";
import { useLibrary } from "@/library/store";
import { newFromArtists } from "@/home/newFromArtists";
import { recommendations, type Recommendations } from "@/home/recommendations";
import { player } from "@/player/engine";
import { IconRadio } from "@/ui/components/Icons";
import { useNavigation } from "@/ui/navigation";

function greeting(date = new Date()): string {
  const h = date.getHours();
  if (h < 5) return "Доброй ночи";
  if (h < 12) return "Доброе утро";
  if (h < 18) return "Добрый день";
  return "Добрый вечер";
}

// SoundCloud charts change slowly: keep them for the session instead of asking every visit
const chartCache = new Map<string, Track[]>();

function useChart(kind: "top" | "trending") {
  const [tracks, setTracks] = useState<Track[]>(() => chartCache.get(kind) ?? []);
  const [state, setState] = useState<"loading" | "ready" | "error">(chartCache.has(kind) ? "ready" : "loading");
  useEffect(() => {
    if (chartCache.has(kind)) return;
    const controller = new AbortController();
    soundCloudCharts(kind, controller.signal)
      .then((list) => {
        chartCache.set(kind, list);
        setTracks(list);
        setState("ready");
      })
      .catch(() => !controller.signal.aborted && setState("error"));
    return () => controller.abort();
  }, [kind]);
  return { tracks, state };
}

function Row({ title, tracks, state, ranked, hint }: { title: string; tracks: Track[]; state: string; ranked?: boolean; hint?: string }) {
  if (state === "ready" && tracks.length === 0) return null;
  return (
    <section className="row">
      <h2 className="row-title">{title}</h2>
      {hint && <p className="muted small row-hint">{hint}</p>}
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

function Recent() {
  const entries = useHistory((s) => s.entries);
  const tracks = useMemo(() => entries.slice(0, 16).map((e) => e.track), [entries]);
  if (!tracks.length) return null;
  return <Row title="Недавно слушали" tracks={tracks} state="ready" />;
}

function SmartRow({ version }: { version: number }) {
  const likes = useLibrary((s) => s.likes);
  const go = useNavigation((s) => s.go);
  const lists = useMemo(
    () => SMART_PLAYLISTS.map((kind) => ({ kind, tracks: getSmartPlaylist(kind, likes.map((l) => l.track)) })).filter((l) => l.tracks.length > 0),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [likes, version],
  );
  if (!lists.length) return null;
  return (
    <section className="row">
      <h2 className="row-title">Собрано для вас</h2>
      <div className="row-scroll">
        {lists.map(({ kind, tracks }) => (
          <button key={kind} type="button" className="smart-card" onClick={() => go("smart", kind)} style={{ "--hue": SMART_INFO[kind].hue } as React.CSSProperties}>
            <span className="smart-art">
              {tracks.slice(0, 4).map((t) => (t.artwork ? <img key={t.id} src={t.artwork} alt="" loading="lazy" /> : <span key={t.id} />))}
              <span className="smart-glow"><IconSparkle size={22} /></span>
            </span>
            <span className="track-card-title">{SMART_INFO[kind].title}</span>
            <span className="track-card-artist">{tracks.length} треков</span>
          </button>
        ))}
      </div>
    </section>
  );
}

function Tops({ version }: { version: number }) {
  const [range, setRange] = useState<StatsRange>("week");
  const go = useNavigation((s) => s.go);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const top = useMemo(() => getTopTracks(range, 10), [range, version]);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const artists = useMemo(() => getTopArtists(range, 10), [range, version]);
  const all = useMemo(() => getTopTracks("all", 1), [version]);
  if (!all.length) return null;
  const tracks = top.map((t) => t.track);
  return (
    <section className="row tops">
      <div className="row-head">
        <h2 className="row-title">Ваш топ-10</h2>
        <Segmented
          label="Период"
          value={range}
          onChange={setRange}
          options={[
            { value: "week", label: "Неделя" },
            { value: "month", label: "Месяц" },
            { value: "all", label: "Всё время" },
          ]}
        />
      </div>
      {top.length === 0 ? (
        <p className="muted">За этот период ещё ничего не слушали.</p>
      ) : (
        <div className="tops-grid">
          <div className="track-list">
            {top.map((row, i) => (
              <TrackRow key={row.track.id} track={row.track} queue={tracks} index={i} number={i + 1} note={playsWord(row.plays)} />
            ))}
          </div>
          <div className="top-artists">
            <h3 className="eyebrow">Ваши топ-исполнители</h3>
            {artists.map((a, i) => (
              <button key={a.id + a.name} type="button" className="top-artist" onClick={() => go("artist", a.id, a.name)}>
                <span className="top-artist-n">{i + 1}</span>
                <span className="top-artist-art">{a.cover ? <img src={a.cover} alt="" loading="lazy" /> : <IconUser size={18} />}</span>
                <span className="top-artist-text">
                  <span className="top-artist-name">{a.name}</span>
                  <span className="muted small">{playsWord(a.plays)} · {formatListenTime(a.ms)}</span>
                </span>
              </button>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

function ForYou() {
  const likes = useLibrary((s) => s.likes);
  const history = useHistory((s) => s.entries);
  const [data, setData] = useState<Recommendations | null>(null);
  const [state, setState] = useState<"loading" | "ready">("loading");
  // recompute when the seeds could have changed, not on every play
  const key = `${likes.length}|${history[0]?.track.id ?? ""}`;
  useEffect(() => {
    if (!likes.length && !history.length) {
      setState("ready");
      return;
    }
    const controller = new AbortController();
    recommendations(likes, history, controller.signal)
      .then((r) => {
        if (controller.signal.aborted) return;
        setData(r);
        setState("ready");
      })
      .catch(() => setState("ready"));
    return () => controller.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  if (state === "ready" && !data?.forYou.length) return null;
  return (
    <>
      <section className="row">
        <div className="row-head">
          <h2 className="row-title">Рекомендации для вас</h2>
          {data?.forYou.length ? (
            <button type="button" className="btn ghost" onClick={() => void player.playQueue(data.forYou)}>
              <IconRadio size={16} /> Слушать подборку
            </button>
          ) : null}
        </div>
        <p className="muted small row-hint">По тому, что вы слушаете и лайкаете</p>
        <div className="row-scroll">
          {state === "loading"
            ? Array.from({ length: 8 }, (_, i) => <span key={i} className="track-card skeleton" />)
            : data!.forYou.map((t, i) => <TrackCard key={t.id} track={t} queue={data!.forYou} index={i} />)}
        </div>
      </section>
      {data?.because.map((b) => (
        <Row key={b.seed.id} title={`Похоже на «${b.seed.title}»`} tracks={b.tracks} state="ready" />
      ))}
    </>
  );
}

function NewFromArtists() {
  const likes = useLibrary((s) => s.likes);
  const [tracks, setTracks] = useState<Track[]>([]);
  const key = likes.length;
  useEffect(() => {
    if (!likes.length) return;
    const controller = new AbortController();
    newFromArtists(likes, controller.signal).then((list) => !controller.signal.aborted && setTracks(list)).catch(() => {});
    return () => controller.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  if (!tracks.length) return null;
  return <Row title="Новое у ваших исполнителей" hint="Свежие треки тех, кого вы чаще всего лайкаете" tracks={tracks} state="ready" />;
}

export default function HomePage() {
  const version = useStatsVersion();
  const trending = useChart("trending");
  const top = useChart("top");
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const today = useMemo(() => listenedToday(), [version]);
  return (
    <div className="page">
      <header className="page-head home-head">
        <h1 className="display">{greeting()}</h1>
        {today >= 60_000 && (
          <span className="today">
            <span className="eyebrow">Сегодня прослушано</span>
            <span className="today-value">{formatListenTime(today)}</span>
          </span>
        )}
      </header>
      <Recent />
      <ForYou />
      <SmartRow version={version} />
      <Tops version={version} />
      <NewFromArtists />
      <Row title="В тренде на SoundCloud" {...trending} />
      <Row title="Топ SoundCloud" {...top} ranked />
    </div>
  );
}
