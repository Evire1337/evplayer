import { useEffect, useMemo, useState } from "react";
import type { Track } from "@/types";
import { soundCloudUser, searchSoundCloud } from "@/services/soundcloud/api";
import { searchYouTubeMusic, youTubeArtist } from "@/services/youtube/music";
import { useNavigation } from "@/ui/navigation";
import TrackRow from "@/ui/components/TrackRow";
import { IconPlay, IconRadio, IconUser } from "@/ui/components/Icons";
import { formatCount } from "@/ui/format";
import { player } from "@/player/engine";
import { getTrackStats } from "@/stats/listeningStats";
import { playsWord } from "@/stats/format";
import { useLocalFiles, recordToTrack } from "@/files/localFiles";

interface ArtistData {
  name: string;
  avatar?: string;
  /** what the service shows: followers on SoundCloud, monthly listeners on YouTube Music */
  facts: string[];
  popular: Track[];
  tracks: Track[];
  source: string;
}

async function loadArtist(id: string, name: string, signal: AbortSignal): Promise<ArtistData> {
  if (id.startsWith("scu:")) {
    const { artist, tracks, popular } = await soundCloudUser(id, signal);
    return {
      name: artist?.name ?? name,
      avatar: artist?.avatar,
      facts: artist?.followers !== undefined ? [`${formatCount(artist.followers)} подписчиков на SoundCloud`] : [],
      popular: (popular.length ? popular : [...tracks].sort((a, b) => (b.plays ?? 0) - (a.plays ?? 0))).slice(0, 10),
      tracks,
      source: "SoundCloud",
    };
  }
  if (id.startsWith("yta:")) {
    const a = await youTubeArtist(id, signal);
    const more = a.tracks.length < 5 ? await searchYouTubeMusic(a.name, signal).catch(() => []) : [];
    const tracks = [...a.tracks, ...more.filter((t) => !a.tracks.some((x) => x.id === t.id))];
    return {
      name: a.name,
      avatar: a.avatar,
      facts: [
        ...(a.monthlyListeners !== undefined ? [`${formatCount(a.monthlyListeners)} слушателей в месяц`] : []),
        ...(a.subscribers !== undefined ? [`${formatCount(a.subscribers)} подписчиков на YouTube`] : []),
      ],
      popular: [...tracks].sort((x, y) => (y.plays ?? 0) - (x.plays ?? 0)).slice(0, 10),
      tracks,
      source: "YouTube Music",
    };
  }
  // your files or a name only: look the name up on both services
  const local = useLocalFiles.getState().records.map(recordToTrack).filter((t) => t.artists.some((a) => a.name.toLowerCase() === name.toLowerCase()));
  const [sc, yt] = await Promise.all([
    searchSoundCloud(name, signal).catch(() => ({ tracks: [] as Track[], artists: [] })),
    searchYouTubeMusic(name, signal).catch(() => [] as Track[]),
  ]);
  const found = [...yt, ...sc.tracks].filter((t) => t.artists.some((a) => a.name.toLowerCase().includes(name.toLowerCase())));
  return {
    name,
    avatar: sc.artists.find((a) => a.name.toLowerCase() === name.toLowerCase())?.avatar,
    facts: local.length ? [`${local.length} в ваших файлах`] : [],
    popular: [...found].sort((a, b) => (b.plays ?? 0) - (a.plays ?? 0)).slice(0, 10),
    tracks: [...local, ...found],
    source: "поиск",
  };
}

export default function ArtistPage() {
  const id = useNavigation((s) => s.playlistId) ?? "";
  const label = useNavigation((s) => s.label) ?? "";
  const [data, setData] = useState<ArtistData | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "error">("loading");

  useEffect(() => {
    const controller = new AbortController();
    setState("loading");
    setData(null);
    loadArtist(id, label, controller.signal)
      .then((d) => {
        setData(d);
        setState("ready");
      })
      .catch(() => !controller.signal.aborted && setState("error"));
    return () => controller.abort();
  }, [id, label]);

  // your own listening of this artist, from the statistics on this computer
  const mine = useMemo(() => {
    if (!data) return { plays: 0, perTrack: new Map<string, number>() };
    const perTrack = new Map<string, number>();
    let plays = 0;
    for (const t of [...data.popular, ...data.tracks]) {
      if (perTrack.has(t.id)) continue;
      const n = getTrackStats(t.id).all.n;
      perTrack.set(t.id, n);
      plays += n;
    }
    return { plays, perTrack };
  }, [data]);

  const name = data?.name ?? label;
  const all = data?.tracks ?? [];
  const popular = data?.popular ?? [];

  return (
    <div className="page">
      <header className="pl-hero artist-hero">
        <span className="artist-photo">{data?.avatar ? <img src={data.avatar} alt="" /> : <IconUser size={64} />}</span>
        <div className="pl-hero-text">
          <span className="eyebrow">Исполнитель{data ? ` · ${data.source}` : ""}</span>
          <h1 className="display">{name}</h1>
          <p className="muted">
            {[...(data?.facts ?? []), ...(mine.plays ? [`у вас ${playsWord(mine.plays)}`] : [])].join(" · ") || (state === "loading" ? "Загружаю…" : "")}
          </p>
          <div className="pl-actions">
            <button type="button" className="round-btn main" aria-label="Слушать" disabled={!all.length} onClick={() => void player.playQueue(popular.length ? popular : all)}>
              <IconPlay size={22} />
            </button>
            {popular[0] && (
              <button type="button" className="btn ghost" onClick={() => void player.startRadio(popular[0])}>
                <IconRadio size={16} /> Радио
              </button>
            )}
          </div>
        </div>
      </header>

      {state === "error" && <p className="muted center">Не получилось загрузить исполнителя.</p>}

      {popular.length > 0 && (
        <section className="row">
          <h2 className="row-title">Популярное</h2>
          <div className="track-list">
            {popular.map((t, i) => (
              <TrackRow
                key={t.id}
                track={t}
                queue={popular}
                index={i}
                number={i + 1}
                note={[t.plays !== undefined ? formatCount(t.plays) : "", mine.perTrack.get(t.id) ? `вы: ${mine.perTrack.get(t.id)}` : ""].filter(Boolean).join(" · ") || undefined}
              />
            ))}
          </div>
        </section>
      )}

      {all.length > popular.length && (
        <section className="row">
          <h2 className="row-title">Все треки</h2>
          <div className="track-list">
            {all.map((t, i) => (
              <TrackRow key={t.id} track={t} queue={all} index={i} showSource />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
