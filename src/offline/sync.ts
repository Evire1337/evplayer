import { useLibrary } from "@/library/store";
import { keepTracks, listenForProgress } from "./keep";
import { offlineSupported, useKeepLists, useOffline } from "./store";
import { tracksOf } from "./scopes";

// Lists marked "keep offline" stay complete: when songs are added to them, or the app
// starts, the missing ones are fetched (but not the ones that could not be kept before).
// Playlists that were deleted drop out of the list.

let timer: ReturnType<typeof setTimeout> | undefined;

function topUp(): void {
  if (!offlineSupported() || !useOffline.getState().loaded) return;
  const { lists, remove } = useKeepLists.getState();
  const known = new Set(["likes", ...useLibrary.getState().playlists.map((p) => p.id)]);
  const gone = lists.filter((l) => !known.has(l));
  if (gone.length) remove(gone);
  const wanted = lists.filter((l) => known.has(l));
  if (wanted.length && navigator.onLine) keepTracks(tracksOf(wanted), { auto: true });
}

const later = () => {
  clearTimeout(timer);
  timer = setTimeout(topUp, 4000);
};

/** call once when the app starts */
export function startOffline(): () => void {
  if (!offlineSupported()) return () => undefined;
  const stopProgress = listenForProgress();
  void useOffline.getState().refresh().then(topUp);
  const stopLibrary = useLibrary.subscribe(later);
  const stopKeep = useKeepLists.subscribe(later);
  window.addEventListener("online", later);
  return () => {
    stopProgress();
    stopLibrary();
    stopKeep();
    window.removeEventListener("online", later);
    clearTimeout(timer);
  };
}
