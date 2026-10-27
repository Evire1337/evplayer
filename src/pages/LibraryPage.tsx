import { useLibrary } from "@/library/store";
import PlaylistCover from "@/library/Cover";
import { openCreatePlaylist, openImportCode } from "@/library/dialogs";
import { useNavigation } from "@/ui/navigation";
import { formatDate } from "@/ui/dates";
import { IconCode, IconFolder, IconPlus } from "@/ui/components/Icons";
import { openSpotifyExport } from "@/library/SpotifyExportDialog";
import { useLocalFiles } from "@/files/localFiles";
import { getSmartPlaylist, SMART_PLAYLISTS } from "@/stats/listeningStats";
import { SMART_INFO } from "@/stats/smart";
import SmartCover from "@/stats/SmartCover";
import { useStatsVersion } from "@/stats/useStats";
import { useEffect, useMemo } from "react";
import { useSettings } from "@/settings/store";
import { IconGridView, IconListView } from "@/ui/components/Icons";

function songs(n: number): string {
  const mod10 = n % 10;
  const mod100 = n % 100;
  const word = mod10 === 1 && mod100 !== 11 ? "трек" : mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14) ? "трека" : "треков";
  return `${n} ${word}`;
}

export { songs };

export default function LibraryPage() {
  const { likes, playlists } = useLibrary();
  const go = useNavigation((s) => s.go);
  const view = useSettings((s) => s.libraryView);
  const files = useLocalFiles((s) => s.records.length);
  const version = useStatsVersion();
  useEffect(() => void useLocalFiles.getState().refresh(), []);
  const smart = useMemo(
    () => SMART_PLAYLISTS.map((kind) => {
        const tracks = getSmartPlaylist(kind, likes.map((l) => l.track));
        return { kind, n: tracks.length, tracks };
      }).filter((x) => x.n > 0),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [likes, version],
  );
  return (
    <div className="page">
      <header className="page-head with-actions">
        <h1 className="display">Библиотека</h1>
        <div className="head-actions">
          <button type="button" className="btn primary" onClick={() => openCreatePlaylist()}>
            <IconPlus size={16} /> Новый плейлист
          </button>
          <button type="button" className="btn ghost" onClick={() => openImportCode()}>
            <IconCode size={16} /> Импорт по ссылке или коду
          </button>
          <button type="button" className="btn ghost" onClick={openSpotifyExport}>
            Из Spotify
          </button>
          <div className="view-toggle" role="radiogroup" aria-label="Вид">
            <button type="button" role="radio" aria-checked={view === "grid"} className={view === "grid" ? "is-on" : ""} aria-label="Крупные значки" data-tip="Крупные значки" onClick={() => useSettings.getState().set({ libraryView: "grid" })}>
              <IconGridView size={17} />
            </button>
            <button type="button" role="radio" aria-checked={view === "list"} className={view === "list" ? "is-on" : ""} aria-label="Списком" data-tip="Списком" onClick={() => useSettings.getState().set({ libraryView: "list" })}>
              <IconListView size={17} />
            </button>
          </div>
        </div>
      </header>

      <div className={view === "list" ? "pl-list" : "pl-grid"}>
        <button type="button" className="pl-card" onClick={() => go("playlist", "likes")}>
          <PlaylistCover liked />
          <span className="pl-card-title">Понравившиеся</span>
          <span className="pl-card-sub">{songs(likes.length)}</span>
        </button>
        <button type="button" className="pl-card" onClick={() => go("files")}>
          <span className="pl-cover files-cover"><IconFolder size={48} /></span>
          <span className="pl-card-title">Мои файлы</span>
          <span className="pl-card-sub">{files ? songs(files) : "MP3, FLAC и другие с компьютера"}</span>
        </button>
        {smart.map(({ kind, n, tracks }) => (
          <button key={kind} type="button" className="pl-card" onClick={() => go("smart", kind)}>
            <SmartCover kind={kind} tracks={tracks} />
            <span className="pl-card-title">{SMART_INFO[kind].title}</span>
            <span className="pl-card-sub">Умный · {songs(n)}</span>
          </button>
        ))}
        {playlists.map((p) => (
          <button key={p.id} type="button" className="pl-card" onClick={() => go("playlist", p.id)}>
            <PlaylistCover playlist={p} />
            <span className="pl-card-title">{p.title}</span>
            <span className="pl-card-sub">{songs(p.items.length)} · создан {formatDate(p.createdAt)}</span>
          </button>
        ))}
      </div>
      {playlists.length === 0 && (
        <p className="muted">Создайте свой первый плейлист: в него можно класть треки из SoundCloud, YouTube и ваших файлов вперемешку. Через правый клик по любому треку → «Добавить в плейлист».</p>
      )}
    </div>
  );
}
