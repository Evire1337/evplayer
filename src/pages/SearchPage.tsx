import { useEffect, useRef, useState } from "react";
import type { ArtistSummary, Track } from "@/types";
import { searchSoundCloud } from "@/services/soundcloud/api";
import { searchYouTubeMusic } from "@/services/youtube/music";
import TrackRow from "@/ui/components/TrackRow";
import { IconClose, IconSearch } from "@/ui/components/Icons";
import { Segmented } from "@/ui/components/Controls";
import { formatCount } from "@/ui/format";
import { useNavigation } from "@/ui/navigation";
import { useSettings, type SearchSource } from "@/settings/store";
import { importFromLink, looksLikeLink } from "@/library/importLink";

const DEBOUNCE_MS = 350;

type Result = { tracks: Track[]; artists: ArtistSummary[]; failed: string[] };

/** one list from both services: take turns so neither buries the other */
export function interleave(a: Track[], b: Track[]): Track[] {
  const out: Track[] = [];
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if (a[i]) out.push(a[i]);
    if (b[i]) out.push(b[i]);
  }
  return out;
}

async function runSearch(q: string, source: SearchSource, signal: AbortSignal): Promise<Result> {
  const failed: string[] = [];
  const sc = source !== "youtube"
    ? searchSoundCloud(q, signal).catch(() => (failed.push("SoundCloud"), { tracks: [], artists: [] }))
    : Promise.resolve({ tracks: [], artists: [] });
  const yt = source !== "soundcloud"
    ? searchYouTubeMusic(q, signal).catch(() => (failed.push("YouTube Music"), [] as Track[]))
    : Promise.resolve([] as Track[]);
  const [scRes, ytRes] = await Promise.all([sc, yt]);
  return { tracks: interleave(scRes.tracks, ytRes), artists: scRes.artists, failed };
}

export default function SearchPage() {
  const defaultSource = useSettings((s) => s.defaultSearchSource);
  const [query, setQuery] = useState("");
  const [source, setSource] = useState<SearchSource>(defaultSource);
  const [kind, setKind] = useState<"all" | "tracks" | "artists">("all");
  const [result, setResult] = useState<Result>({ tracks: [], artists: [], failed: [] });
  const [state, setState] = useState<"idle" | "loading" | "ready" | "error">("idle");
  const inputRef = useRef<HTMLInputElement>(null);
  const go = useNavigation((s) => s.go);
  const link = looksLikeLink(query);

  useEffect(() => inputRef.current?.focus(), []);

  useEffect(() => {
    const q = query.trim();
    if (q.length < 2 || looksLikeLink(q)) {
      setState("idle");
      return;
    }
    const controller = new AbortController();
    setState("loading");
    const timer = setTimeout(() => {
      runSearch(q, source, controller.signal)
        .then((res) => {
          if (controller.signal.aborted) return;
          setResult(res);
          setState(res.tracks.length === 0 && res.failed.length ? "error" : "ready");
        })
        .catch(() => !controller.signal.aborted && setState("error"));
    }, DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [query, source]);

  const { tracks, artists } = result;
  const showArtists = kind !== "tracks" && artists.length > 0 && state !== "idle";
  const showTracks = kind !== "artists" && tracks.length > 0 && state !== "idle";

  return (
    <div className="page">
      <label className="search-field glass" htmlFor="search">
        <IconSearch size={19} />
        <input
          id="search"
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && link && void importFromLink(query.trim())}
          placeholder="Трек, исполнитель или ссылка на плейлист"
          autoComplete="off"
          spellCheck={false}
        />
        {query && (
          <button type="button" className="icon-btn" onClick={() => setQuery("")} aria-label="Очистить">
            <IconClose size={16} />
          </button>
        )}
      </label>

      <div className="search-filters">
        <Segmented
          label="Где искать"
          value={source}
          onChange={setSource}
          options={[
            { value: "all", label: "Везде" },
            { value: "soundcloud", label: "SoundCloud" },
            { value: "youtube", label: "YouTube Music" },
          ]}
        />
        <Segmented
          label="Что показывать"
          value={kind}
          onChange={setKind}
          options={[
            { value: "all", label: "Всё" },
            { value: "tracks", label: "Треки" },
            { value: "artists", label: "Исполнители" },
          ]}
        />
      </div>

      {link && (
        <div className="link-hint glass">
          <span>Это ссылка. Добавить её содержимое в библиотеку как плейлист?</span>
          <button type="button" className="btn primary" onClick={() => void importFromLink(query.trim())}>Импортировать</button>
        </div>
      )}
      {state === "idle" && !link && <p className="muted center">Ищите на SoundCloud и YouTube Music сразу — или вставьте ссылку на плейлист из Spotify, YouTube, SoundCloud или Deezer.</p>}
      {state === "error" && <p className="muted center">Не получилось найти. Проверьте интернет и попробуйте ещё раз.</p>}
      {state === "ready" && result.failed.length > 0 && <p className="muted small">{result.failed.join(" и ")} сейчас не отвечает — показаны остальные результаты.</p>}
      {state === "ready" && tracks.length === 0 && artists.length === 0 && <p className="muted center">Ничего не нашлось.</p>}

      {showArtists && (
        <section className="row">
          <h2 className="row-title">Исполнители</h2>
          <div className="row-scroll">
            {artists.map((a) => (
              <button key={a.id} type="button" className="artist-chip" onClick={() => go("artist", a.id, a.name)}>
                <span className="artist-avatar">{a.avatar ? <img src={a.avatar} alt="" loading="lazy" /> : null}</span>
                <span className="artist-name">{a.name}</span>
                {a.followers !== undefined && <span className="muted small">{formatCount(a.followers)} подписчиков</span>}
              </button>
            ))}
          </div>
        </section>
      )}

      {showTracks && (
        <section className="row">
          <h2 className="row-title">Треки</h2>
          <div className={`track-list${state === "loading" ? " is-stale" : ""}`}>
            {tracks.map((t, i) => (
              <TrackRow key={t.id} track={t} queue={tracks} index={i} showSource />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
