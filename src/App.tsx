import { currentTrack, usePlayer } from "@/player/store";
import Backdrop from "@/ui/backdrop/Backdrop";
import Sidebar from "@/ui/components/Sidebar";
import PlayerBar from "@/ui/components/PlayerBar";
import UpdateToast from "@/ui/components/UpdateToast";
import { useNavigation } from "@/ui/navigation";
import HomePage from "@/pages/HomePage";
import SearchPage from "@/pages/SearchPage";
import LibraryPage from "@/pages/LibraryPage";

export default function App() {
  const page = useNavigation((s) => s.page);
  const artwork = usePlayer((s) => currentTrack(s)?.artwork);
  const playing = usePlayer((s) => s.status === "playing");
  return (
    <div className="app">
      <Backdrop image={artwork} playing={playing} />
      <div className="titlebar" />
      <Sidebar />
      <main className="content">
        {page === "home" && <HomePage />}
        {page === "search" && <SearchPage />}
        {page === "library" && <LibraryPage />}
      </main>
      <PlayerBar />
      <UpdateToast />
    </div>
  );
}
