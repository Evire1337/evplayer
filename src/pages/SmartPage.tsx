import { useMemo } from "react";
import { useNavigation } from "@/ui/navigation";
import { useLibrary } from "@/library/store";
import { getSmartPlaylist, type SmartPlaylistKind } from "@/stats/listeningStats";
import { SMART_INFO } from "@/stats/smart";
import { useStatsVersion } from "@/stats/useStats";
import TrackRow from "@/ui/components/TrackRow";
import { IconPlay, IconShuffle, IconSparkle } from "@/ui/components/Icons";
import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { openCreatePlaylist } from "@/library/dialogs";

export default function SmartPage() {
  const kind = (useNavigation((s) => s.playlistId) ?? "on_repeat") as SmartPlaylistKind;
  const likes = useLibrary((s) => s.likes);
  const version = useStatsVersion();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const tracks = useMemo(() => getSmartPlaylist(kind, likes.map((l) => l.track)), [kind, likes, version]);
  const info = SMART_INFO[kind] ?? SMART_INFO.on_repeat;
  return (
    <div className="page">
      <header className="pl-hero">
        <span className="pl-cover smart-cover" style={{ "--hue": info.hue } as React.CSSProperties}><IconSparkle size={56} /></span>
        <div className="pl-hero-text">
          <span className="eyebrow">Умный плейлист · обновляется сам</span>
          <h1 className="display">{info.title}</h1>
          <p className="muted">{info.description} · {tracks.length} треков</p>
          <div className="pl-actions">
            <button type="button" className="round-btn main" aria-label="Слушать" disabled={!tracks.length} onClick={() => void player.playQueue(tracks)}>
              <IconPlay size={22} />
            </button>
            <button
              type="button"
              className="round-btn soft"
              aria-label="Перемешать"
              disabled={!tracks.length}
              onClick={() => {
                if (!usePlayer.getState().shuffle) player.toggleShuffle();
                void player.playQueue(tracks, Math.floor(Math.random() * tracks.length));
              }}
            >
              <IconShuffle size={18} />
            </button>
            <button type="button" className="btn ghost" disabled={!tracks.length} onClick={() => openCreatePlaylist(tracks)}>Сохранить как плейлист</button>
          </div>
        </div>
      </header>
      {tracks.length === 0 ? (
        <p className="muted center">Пока пусто — слушайте музыку, и плейлист наполнится сам.</p>
      ) : (
        <div className="track-list">
          {tracks.map((t, i) => (
            <TrackRow key={t.id} track={t} queue={tracks} index={i} showSource />
          ))}
        </div>
      )}
    </div>
  );
}
