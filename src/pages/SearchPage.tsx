import { useEffect, useRef, useState } from "react";
import type { ArtistSummary, Track } from "@/types";
import { searchSoundCloud } from "@/services/soundcloud/api";
import TrackRow from "@/ui/components/TrackRow";
import { IconClose, IconSearch } from "@/ui/components/Icons";
import { formatCount } from "@/ui/format";

const DEBOUNCE_MS = 350;

export default function SearchPage() {
  const [query, setQuery] = useState("");
  const [tracks, setTracks] = useState<Track[]>([]);
  const [artists, setArtists] = useState<ArtistSummary[]>([]);
  const [state, setState] = useState<"idle" | "loading" | "ready" | "error">("idle");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => inputRef.current?.focus(), []);

  useEffect(() => {
    const q = query.trim();
    if (q.length < 2) {
      setState("idle");
      return;
    }
    const controller = new AbortController();
    setState("loading");
    const timer = setTimeout(() => {
      searchSoundCloud(q, controller.signal)
        .then((res) => {
          setTracks(res.tracks);
          setArtists(res.artists);
          setState("ready");
        })
        .catch(() => !controller.signal.aborted && setState("error"));
    }, DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [query]);

  return (
    <div className="page">
      <label className="search-field glass" htmlFor="search">
        <IconSearch size={19} />
        <input
          id="search"
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Треки и исполнители на SoundCloud"
          autoComplete="off"
          spellCheck={false}
        />
        {query && (
          <button type="button" className="icon-btn" onClick={() => setQuery("")} aria-label="Очистить">
            <IconClose size={16} />
          </button>
        )}
      </label>

      {state === "idle" && <p className="muted center">Начните вводить название трека или имя исполнителя.</p>}
      {state === "error" && <p className="muted center">Не получилось найти. Проверьте интернет и попробуйте ещё раз.</p>}
      {state === "ready" && tracks.length === 0 && artists.length === 0 && <p className="muted center">Ничего не нашлось.</p>}

      {artists.length > 0 && state !== "idle" && (
        <section className="row">
          <h2 className="row-title">Исполнители</h2>
          <div className="row-scroll">
            {artists.map((a) => (
              <div key={a.id} className="artist-chip">
                <span className="artist-avatar">{a.avatar ? <img src={a.avatar} alt="" loading="lazy" /> : null}</span>
                <span className="artist-name">{a.name}</span>
                {a.followers !== undefined && <span className="muted small">{formatCount(a.followers)} подписчиков</span>}
              </div>
            ))}
          </div>
        </section>
      )}

      {tracks.length > 0 && state !== "idle" && (
        <section className="row">
          <h2 className="row-title">Треки</h2>
          <div className={`track-list${state === "loading" ? " is-stale" : ""}`}>
            {tracks.map((t, i) => (
              <TrackRow key={t.id} track={t} queue={tracks} index={i} />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
