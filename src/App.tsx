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

export default function App() {
  const page = useNavigation((s) => s.page);
  const artwork = usePlayer((s) => currentTrack(s)?.artwork);
  const playing = usePlayer((s) => s.status === "playing");
  return (
    <div className={`app page-${page}`}>
      <Backdrop image={artwork} playing={playing} />
      <div className="titlebar" />
      <Rail />
      <main className="content">
        {page === "now" && <NowPlayingPage />}
        {page === "home" && <HomePage />}
        {page === "search" && <SearchPage />}
        {page === "library" && <LibraryPage />}
      </main>
      {page !== "now" && <MiniPlayer />}
      <UpdateToast />
    </div>
  );
}
