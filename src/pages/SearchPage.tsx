import RowScroll from "@/ui/components/RowScroll";
import { useEffect, useRef } from "react";
import type { CollectionSummary, Track } from "@/types";
import { searchSoundCloud, searchSoundCloudCollections } from "@/services/soundcloud/api";
import { searchYouTubeCollections, searchYouTubeMusic } from "@/services/youtube/music";
import CollectionCard from "@/ui/components/CollectionCard";
import TrackRow from "@/ui/components/TrackRow";
import { IconClose, IconSearch } from "@/ui/components/Icons";
import { Segmented } from "@/ui/components/Controls";
import { formatCount } from "@/ui/format";
import { useNavigation } from "@/ui/navigation";
import { useSettings, type SearchSource } from "@/settings/store";
import { importFromLink, looksLikeLink } from "@/library/importLink";
import { searchKey, useSearchHistory, useSearchState, type SearchKind, type SearchResult } from "./searchState";

const DEBOUNCE_MS = 350;

type Result = SearchResult;
type Kind = SearchKind;
/** a query that stayed on screen with results this long counts as a search for the history */
const HISTORY_AFTER_MS = 2500;

function mix<T>(a: T[], b: T[]): T[] {
  const out: T[] = [];
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if (a[i]) out.push(a[i]);
    if (b[i]) out.push(b[i]);
  }
  return out;
}

/** one list from both services: take turns so neither buries the other */
export function interleave(a: Track[], b: Track[]): Track[] {
  const out: Track[] = [];
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if (a[i]) out.push(a[i]);
    if (b[i]) out.push(b[i]);
  }
  return out;
}

async function collections(q: string, source: SearchSource, kind: "album" | "playlist", signal: AbortSignal): Promise<CollectionSummary[]> {
  const [sc, yt] = await Promise.all([
    source !== "youtube" ? searchSoundCloudCollections(q, kind, signal).catch(() => []) : Promise.resolve([]),
    source !== "soundcloud" ? searchYouTubeCollections(q, kind, signal).catch(() => []) : Promise.resolve([]),
  ]);
  return mix(yt, sc);
}

async function runSearch(q: string, source: SearchSource, kind: Kind, signal: AbortSignal): Promise<Result> {
  if (kind === "albums" || kind === "playlists") {
    const list = await collections(q, source, kind === "albums" ? "album" : "playlist", signal);
    return { tracks: [], artists: [], albums: kind === "albums" ? list : [], playlists: kind === "playlists" ? list : [], failed: [] };
  }
  const failed: string[] = [];
  const sc = source !== "youtube"
    ? searchSoundCloud(q, signal).catch(() => (failed.push("SoundCloud"), { tracks: [], artists: [] }))
    : Promise.resolve({ tracks: [], artists: [] });
  const yt = source !== "soundcloud"
    ? searchYouTubeMusic(q, signal).catch(() => (failed.push("YouTube Music"), [] as Track[]))
    : Promise.resolve([] as Track[]);
  const albums = kind === "all" ? collections(q, source, "album", signal) : Promise.resolve([]);
  const [scRes, ytRes, albumRes] = await Promise.all([sc, yt, albums]);
  return { tracks: interleave(scRes.tracks, ytRes), artists: scRes.artists, albums: albumRes, playlists: [], failed };
}

export default function SearchPage() {
  const defaultSource = useSettings((s) => s.defaultSearchSource);
  // kept outside the page: coming back from a song's player finds the same search
  const query = useSearchState((s) => s.query);
  const source = useSearchState((s) => s.source) ?? defaultSource;
  const kind = useSearchState((s) => s.kind);
  const result = useSearchState((s) => s.result);
  const state = useSearchState((s) => s.status);
  const save = useSearchState((s) => s.set);
  const setQuery = (q: string) => save({ query: q });
  const setSource = (v: SearchSource) => save({ source: v });
  const setKind = (v: Kind) => save({ kind: v });
  const setState = (st: "idle" | "loading" | "ready" | "error") => save({ status: st });
  const history = useSearchHistory((s) => s.queries);
  const inputRef = useRef<HTMLInputElement>(null);
  const go = useNavigation((s) => s.go);
  const link = looksLikeLink(query);

  // a fresh visit puts the cursor in the field; coming back to results does not (on a phone
  // the keyboard would cover them)
  useEffect(() => {
    if (!useSearchState.getState().query) inputRef.current?.focus();
  }, []);

  // a new query starts its results from the top, not under the field where the old ones were
  const typed = useRef(false);
  useEffect(() => {
    if (typed.current) inputRef.current?.closest(".content")?.scrollTo({ top: 0 });
    typed.current = true;
  }, [query]);

  useEffect(() => {
    const q = query.trim();
    if (q.length < 2 || looksLikeLink(q)) {
      setState("idle");
      return;
    }
    const key = searchKey(q, source, kind);
    // the same search as before (back from another tab): the results are still here
    const kept = useSearchState.getState();
    if (kept.resultKey === key && kept.status === "ready") return;
    const controller = new AbortController();
    setState("loading");
    const timer = setTimeout(() => {
      runSearch(q, source, kind, controller.signal)
        .then((res) => {
          if (controller.signal.aborted) return;
          save({
            result: res,
            resultKey: key,
            status: res.tracks.length + res.albums.length + res.playlists.length === 0 && res.failed.length ? "error" : "ready",
          });
        })
        .catch(() => !controller.signal.aborted && setState("error"));
    }, DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, source, kind]);

  // a search that found something and stayed a moment goes into the history
  useEffect(() => {
    if (state !== "ready" || result.tracks.length + result.artists.length + result.albums.length === 0) return;
    const timer = setTimeout(() => useSearchHistory.getState().add(query), HISTORY_AFTER_MS);
    return () => clearTimeout(timer);
  }, [state, query, result]);

  const { tracks, artists, albums, playlists } = result;
  const active = state !== "idle";
  const showArtists = (kind === "all" || kind === "artists") && artists.length > 0 && active;
  const showTracks = (kind === "all" || kind === "tracks") && tracks.length > 0 && active;
  const shownAlbums = active && (kind === "all" || kind === "albums") ? albums : [];
  const shownPlaylists = active && kind === "playlists" ? playlists : [];
  const nothing = tracks.length + artists.length + albums.length + playlists.length === 0;

  return (
    <div className="page">
      <label className="search-field glass" htmlFor="search">
        <IconSearch size={19} />
        <input
          id="search"
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key !== "Enter") return;
            if (link) void importFromLink(query.trim());
            else useSearchHistory.getState().add(query);
          }}
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
            { value: "albums", label: "Альбомы" },
            { value: "playlists", label: "Плейлисты" },
          ]}
        />
      </div>

      {link && (
        <div className="link-hint glass">
          <span>Это ссылка. Добавить её содержимое в библиотеку как плейлист?</span>
          <button type="button" className="btn primary" onClick={() => void importFromLink(query.trim())}>Импортировать</button>
        </div>
      )}
      {state === "idle" && !link && !query.trim() && history.length > 0 && (
        <section className="search-history">
          <div className="row-head">
            <h2 className="row-title">Недавние запросы</h2>
            <button type="button" className="reset-btn" onClick={() => useSearchHistory.getState().clear()}>Очистить</button>
          </div>
          <ul className="search-history-list">
            {history.map((q) => (
              <li key={q}>
                <button type="button" className="search-history-item" onClick={() => setQuery(q)}>
                  <IconSearch size={15} />
                  <span>{q}</span>
                </button>
                <button type="button" className="icon-btn search-history-remove" aria-label={`Убрать «${q}» из истории`} onClick={() => useSearchHistory.getState().remove(q)}>
                  <IconClose size={14} />
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
      {state === "idle" && !link && (query.trim() || history.length === 0) && <p className="muted center">Ищите на SoundCloud и YouTube Music сразу — или вставьте ссылку на плейлист из Spotify, YouTube, SoundCloud или Deezer.</p>}
      {state === "error" && <p className="muted center">Не получилось найти. Проверьте интернет и попробуйте ещё раз.</p>}
      {state === "ready" && result.failed.length > 0 && <p className="muted small">{result.failed.join(" и ")} сейчас не отвечает — показаны остальные результаты.</p>}
      {state === "ready" && nothing && <p className="muted center">Ничего не нашлось.</p>}

      {showArtists && (
        <section className="row">
          <h2 className="row-title">Исполнители</h2>
          <RowScroll>
            {artists.map((a) => (
              <button key={a.id} type="button" className="artist-chip" onClick={() => go("artist", a.id, a.name)}>
                <span className="artist-avatar">{a.avatar ? <img src={a.avatar} alt="" loading="lazy" /> : null}</span>
                <span className="artist-name">{a.name}</span>
                {a.followers !== undefined && <span className="muted small">{formatCount(a.followers)} подписчиков</span>}
              </button>
            ))}
          </RowScroll>
        </section>
      )}

      {shownAlbums.length > 0 && (
        <section className="row">
          <h2 className="row-title">Альбомы</h2>
          {kind === "albums" ? (
            <div className="card-grid">{shownAlbums.map((a) => <CollectionCard key={a.id} item={a} />)}</div>
          ) : (
            <RowScroll>{shownAlbums.map((a) => <CollectionCard key={a.id} item={a} />)}</RowScroll>
          )}
        </section>
      )}

      {shownPlaylists.length > 0 && (
        <section className="row">
          <h2 className="row-title">Плейлисты</h2>
          <div className="card-grid">
            {shownPlaylists.map((a) => <CollectionCard key={a.id} item={a} />)}
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
