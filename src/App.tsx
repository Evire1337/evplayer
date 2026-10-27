import { currentTrack, usePlayer } from "@/player/store";
import Backdrop from "@/ui/backdrop/Backdrop";
import Rail from "@/ui/components/Rail";
import MiniPlayer from "@/ui/components/MiniPlayer";
import UpdateToast from "@/ui/components/UpdateToast";
import { useNavigation } from "@/ui/navigation";
import HomePage from "@/pages/HomePage";
const SearchPage = lazy(() => import("@/pages/SearchPage"));
const LibraryPage = lazy(() => import("@/pages/LibraryPage"));
import NowPlayingPage from "@/pages/NowPlayingPage";
const PlaylistPage = lazy(() => import("@/pages/PlaylistPage"));
const FilesPage = lazy(() => import("@/pages/FilesPage"));
const SettingsPage = lazy(() => import("@/pages/SettingsPage"));
const ArtistPage = lazy(() => import("@/pages/ArtistPage"));
const SmartPage = lazy(() => import("@/pages/SmartPage"));
const CollectionPage = lazy(() => import("@/pages/CollectionPage"));
import ContextMenu from "@/ui/components/ContextMenu";
import Toasts from "@/ui/components/Toasts";
import ImportPill from "@/ui/components/ImportPill";
import Tooltip from "@/ui/components/Tooltip";
import LyricsTracker from "@/ui/components/LyricsTracker";
import { lazy, Suspense, useEffect, useState } from "react";
import LibraryDialogs from "@/library/LibraryDialogs";
import SpotifyExportDialog from "@/library/SpotifyExportDialog";
import TrackStatsDialog from "@/stats/TrackStatsDialog";
import TagEditorDialog from "@/files/tagEditor";
import { useSettings } from "@/settings/store";
import { IconBack } from "@/ui/components/Icons";
import WindowControls from "@/ui/components/WindowControls";
import { useVisible } from "@/app/visibility";

/** after this long out of sight the window lets go of its pages (sooner with memory saver) */
const SLEEP_AFTER_MS = 20_000;
const SAVER_SLEEP_AFTER_MS = 4_000;

function useAsleep(): boolean {
  const visible = useVisible((s) => s.visible);
  const saver = useSettings((s) => s.memorySaver);
  const [asleep, setAsleep] = useState(false);
  useEffect(() => {
    if (visible) {
      setAsleep(false);
      return;
    }
    const timer = setTimeout(() => setAsleep(true), saver ? SAVER_SLEEP_AFTER_MS : SLEEP_AFTER_MS);
    return () => clearTimeout(timer);
  }, [visible, saver]);
  return asleep;
}

export default function App() {
  const page = useNavigation((s) => s.page);
  const canGoBack = useNavigation((s) => s.back.length > 0);
  const goBack = useNavigation((s) => s.goBack);
  const artwork = usePlayer((s) => currentTrack(s)?.artwork);
  const playing = usePlayer((s) => s.status === "playing");
  const lyricsSize = useSettings((s) => s.lyricsSize);
  const lyricsBlur = useSettings((s) => s.lyricsBlur);
  const glow = useSettings((s) => s.coverGlow);
  const overlayLyrics = useSettings((s) => s.overlayEnabled && s.overlayShowLyrics);
  const [overlayOpen, setOverlayOpen] = useState(false);
  useEffect(() => window.evplayer?.overlay.onVisible(setOverlayOpen), []);
  const asleep = useAsleep();
  const saver = useSettings((s) => s.memorySaver);
  useEffect(() => {
    document.documentElement.classList.toggle("saver", saver);
  }, [saver]);
  // hidden in the tray or minimized for a while: the pages, pictures and the backdrop are
  // taken off the page so their memory can go back to the system; the player keeps playing
  if (asleep) return overlayLyrics && overlayOpen ? <LyricsTracker /> : null;
  return (
    <div className={`app page-${page} lyrics-${lyricsSize}${lyricsBlur ? "" : " no-lyrics-blur"}${glow ? "" : " no-glow"}`}>
      {/* memory saver: no WebGL backdrop at all, a still gradient instead */}
      {saver ? <div className="backdrop backdrop-still" aria-hidden="true" /> : <Backdrop image={artwork} playing={playing} />}
      <div className="titlebar" data-tauri-drag-region>
        {canGoBack && page !== "now" && (
          <button type="button" className="back-btn" aria-label="Назад" onClick={goBack}>
            <IconBack size={18} />
          </button>
        )}
        <WindowControls />
      </div>
      <Rail />
      <main className="content">
        {/* pages other than Home and Now playing load the first time they are opened */}
        <Suspense fallback={<div className="page-loading" />}>
        {page === "now" && <NowPlayingPage />}
        {page === "home" && <HomePage />}
        {page === "search" && <SearchPage />}
        {page === "library" && <LibraryPage />}
        {page === "playlist" && <PlaylistPage />}
        {page === "files" && <FilesPage />}
        {page === "settings" && <SettingsPage />}
        {page === "artist" && <ArtistPage />}
        {page === "smart" && <SmartPage />}
        {page === "collection" && <CollectionPage />}
        </Suspense>
      </main>
      {page !== "now" && <MiniPlayer />}
      <UpdateToast />
      <ImportPill />
      <Toasts />
      <ContextMenu />
      <Tooltip />
      {overlayLyrics && overlayOpen && <LyricsTracker />}
      <LibraryDialogs />
      <SpotifyExportDialog />
      <TrackStatsDialog />
      <TagEditorDialog />
    </div>
  );
}
