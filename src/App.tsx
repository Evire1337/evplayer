import { currentTrack, usePlayer } from "@/player/store";
import Backdrop from "@/ui/backdrop/Backdrop";
import Rail from "@/ui/components/Rail";
import MiniPlayer from "@/ui/components/MiniPlayer";
import UpdateToast from "@/ui/components/UpdateToast";
import { useNavigation } from "@/ui/navigation";
import HomePage from "@/pages/HomePage";
import SearchPage from "@/pages/SearchPage";
import LibraryPage from "@/pages/LibraryPage";
import NowPlayingPage from "@/pages/NowPlayingPage";
import PlaylistPage from "@/pages/PlaylistPage";
import FilesPage from "@/pages/FilesPage";
import SettingsPage from "@/pages/SettingsPage";
import ArtistPage from "@/pages/ArtistPage";
import SmartPage from "@/pages/SmartPage";
import ContextMenu from "@/ui/components/ContextMenu";
import Toasts from "@/ui/components/Toasts";
import ImportPill from "@/ui/components/ImportPill";
import LibraryDialogs from "@/library/LibraryDialogs";
import SpotifyExportDialog from "@/library/SpotifyExportDialog";
import TrackStatsDialog from "@/stats/TrackStatsDialog";
import TagEditorDialog from "@/files/tagEditor";
import { useSettings } from "@/settings/store";
import { IconBack } from "@/ui/components/Icons";

export default function App() {
  const page = useNavigation((s) => s.page);
  const canGoBack = useNavigation((s) => s.back.length > 0);
  const goBack = useNavigation((s) => s.goBack);
  const artwork = usePlayer((s) => currentTrack(s)?.artwork);
  const playing = usePlayer((s) => s.status === "playing");
  const lyricsSize = useSettings((s) => s.lyricsSize);
  const lyricsBlur = useSettings((s) => s.lyricsBlur);
  const glow = useSettings((s) => s.coverGlow);
  return (
    <div className={`app page-${page} lyrics-${lyricsSize}${lyricsBlur ? "" : " no-lyrics-blur"}${glow ? "" : " no-glow"}`}>
      <Backdrop image={artwork} playing={playing} />
      <div className="titlebar">
        {canGoBack && page !== "now" && (
          <button type="button" className="back-btn" aria-label="Назад" onClick={goBack}>
            <IconBack size={18} />
          </button>
        )}
      </div>
      <Rail />
      <main className="content">
        {page === "now" && <NowPlayingPage />}
        {page === "home" && <HomePage />}
        {page === "search" && <SearchPage />}
        {page === "library" && <LibraryPage />}
        {page === "playlist" && <PlaylistPage />}
        {page === "files" && <FilesPage />}
        {page === "settings" && <SettingsPage />}
        {page === "artist" && <ArtistPage />}
        {page === "smart" && <SmartPage />}
      </main>
      {page !== "now" && <MiniPlayer />}
      <UpdateToast />
      <ImportPill />
      <Toasts />
      <ContextMenu />
      <LibraryDialogs />
      <SpotifyExportDialog />
      <TrackStatsDialog />
      <TagEditorDialog />
    </div>
  );
}
