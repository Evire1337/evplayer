import VirtualList from "@/ui/components/VirtualList";
import { useEffect, useState } from "react";
import type { Track } from "@/types";
import { useNavigation } from "@/ui/navigation";
import { soundCloudCollection } from "@/services/soundcloud/api";
import { youTubeCollection } from "@/services/youtube/music";
import TrackRow from "@/ui/components/TrackRow";
import { IconDisc, IconPlay, IconPlus, IconShuffle } from "@/ui/components/Icons";
import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { useLibrary } from "@/library/store";
import { toast } from "@/ui/toasts";
import { formatTime } from "@/ui/format";
import { songs } from "./LibraryPage";

interface Loaded {
  title: string;
  kind: "album" | "playlist";
  subtitle?: string;
  artwork?: string;
  artist?: { id: string; name: string };
  source: string;
  tracks: Track[];
}

async function load(id: string, signal: AbortSignal): Promise<Loaded> {
  if (id.startsWith("scp:")) {
    const { summary, tracks } = await soundCloudCollection(id, signal);
    return {
      title: summary?.title ?? "SoundCloud",
      kind: summary?.kind ?? "playlist",
      subtitle: [summary?.subtitle, summary?.year].filter(Boolean).join(" · "),
      artwork: summary?.artwork ?? tracks[0]?.artwork,
      artist: summary?.artist,
      source: "SoundCloud",
      tracks,
    };
  }
  const c = await youTubeCollection(id, signal);
  return { title: c.title, kind: id.startsWith("ytb:MPRE") ? "album" : "playlist", subtitle: c.subtitle, artwork: c.artwork, artist: c.artist, source: "YouTube Music", tracks: c.tracks };
}

export default function CollectionPage() {
  const id = useNavigation((s) => s.playlistId) ?? "";
  const label = useNavigation((s) => s.label) ?? "";
  const go = useNavigation((s) => s.go);
  const [data, setData] = useState<Loaded | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "error">("loading");

  useEffect(() => {
    const controller = new AbortController();
    setState("loading");
    setData(null);
    load(id, controller.signal)
      .then((d) => {
        setData(d);
        setState("ready");
      })
      .catch(() => !controller.signal.aborted && setState("error"));
    return () => controller.abort();
  }, [id]);

  const tracks = data?.tracks ?? [];
  const total = tracks.reduce((sum, t) => sum + t.durationMs, 0);
  return (
    <div className="page">
      <header className="pl-hero">
        <span className="pl-cover hero-cover">{data?.artwork ? <img src={data.artwork} alt="" /> : <IconDisc size={56} />}</span>
        <div className="pl-hero-text">
          <span className="eyebrow">{data ? `${data.kind === "album" ? "Альбом" : "Плейлист"} · ${data.source}` : "Загружаю…"}</span>
          <h1 className="display">{data?.title ?? label}</h1>
          <p className="muted">
            {data?.artist && (
              <>
                <button type="button" className="link-btn strong" onClick={() => go("artist", data.artist!.id, data.artist!.name)}>{data.artist.name}</button>
                {" · "}
              </>
            )}
            {[data?.subtitle, tracks.length ? songs(tracks.length) : "", total ? formatTime(total) : ""].filter(Boolean).join(" · ")}
          </p>
          <div className="pl-actions">
            <button type="button" className="round-btn main" aria-label="Слушать" data-tip="Слушать" disabled={!tracks.length} onClick={() => void player.playQueue(tracks)}>
              <IconPlay size={22} />
            </button>
            <button
              type="button"
              className="round-btn soft"
              aria-label="Вперемешку"
              data-tip="Вперемешку"
              disabled={!tracks.length}
              onClick={() => {
                if (!usePlayer.getState().shuffle) player.toggleShuffle();
                void player.playQueue(tracks, Math.floor(Math.random() * tracks.length));
              }}
            >
              <IconShuffle size={18} />
            </button>
            <button
              type="button"
              className="btn ghost"
              disabled={!tracks.length}
              onClick={() => {
                const lib = useLibrary.getState();
                const p = lib.createPlaylist(data!.title, `${data!.kind === "album" ? "Альбом" : "Плейлист"} из ${data!.source}`);
                if (data?.artwork && /^https:/.test(data.artwork)) lib.updatePlaylist(p.id, { icon: data.artwork });
                lib.addToPlaylist(p.id, tracks);
                toast(`«${data!.title}» сохранён в библиотеку`);
              }}
            >
              <IconPlus size={16} /> В библиотеку
            </button>
          </div>
        </div>
      </header>
      {state === "error" && <p className="muted center">Не получилось открыть.</p>}
      {state === "loading" && <p className="muted center">Загружаю треки…</p>}
      <div className="track-list">
        <VirtualList items={tracks} getKey={(t, i) => t.id + i} render={(t, i) => <TrackRow track={t} queue={tracks} index={i} number={data?.kind === "album" ? i + 1 : undefined} />} />
      </div>
    </div>
  );
}
