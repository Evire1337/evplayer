import { useLibrary } from "./store";

// A second copy of likes and playlists in a file (the desktop side writes it). The browser
// storage reaches the disk with a delay, so a like added just before closing, updating or a
// crash could be gone on the next start. Every change stamps the library with the time and
// writes the file; on start, if the file is newer than the storage, the file wins.

const SAVE_DELAY_MS = 1000;

interface Copy {
  savedAt: number;
  likes: unknown[];
  playlists: unknown[];
}

/** the stored copy as an object, or null when it is missing or broken */
export function parseCopy(json: string | null | undefined): Copy | null {
  if (!json) return null;
  try {
    const c = JSON.parse(json) as Partial<Copy>;
    if (typeof c.savedAt !== "number" || !Array.isArray(c.likes) || !Array.isArray(c.playlists)) return null;
    return { savedAt: c.savedAt, likes: c.likes, playlists: c.playlists };
  } catch {
    return null;
  }
}

/** whether the copy has changes the storage lacks */
export function copyIsNewer(copy: Copy, localSavedAt: number | undefined): boolean {
  return copy.savedAt > (localSavedAt ?? 0);
}

/** on start, before the pages read the library: take the file if the storage is behind */
export async function restoreLibraryCopy(): Promise<void> {
  const api = window.evplayer?.libraryCopy;
  if (!api) return;
  try {
    const copy = parseCopy(await api.load());
    if (!copy || !copyIsNewer(copy, useLibrary.getState().savedAt)) return;
    useLibrary.setState({ likes: copy.likes as never, playlists: copy.playlists as never, savedAt: copy.savedAt });
  } catch (error) {
    console.warn("[library copy]", error);
  }
}

let timer: ReturnType<typeof setTimeout> | undefined;

function write(): void {
  const { likes, playlists, savedAt } = useLibrary.getState();
  void window.evplayer?.libraryCopy.save(JSON.stringify({ savedAt: savedAt ?? Date.now(), likes, playlists })).catch((e) => console.warn("[library copy]", e));
}

/** after every change: stamp the time, write the file a moment later */
export function startLibraryCopy(): void {
  if (!window.evplayer?.libraryCopy) return;
  useLibrary.subscribe((s, prev) => {
    if (s.likes === prev.likes && s.playlists === prev.playlists) return;
    // the stamp itself is a change of the storage but not of likes: it does not come back here
    if (s.savedAt === prev.savedAt) useLibrary.setState({ savedAt: Date.now() });
    clearTimeout(timer);
    timer = setTimeout(write, SAVE_DELAY_MS);
  });
  window.addEventListener("beforeunload", write);
}
