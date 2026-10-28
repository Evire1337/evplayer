import { useNavigation } from "@/ui/navigation";

// Where a queue was started from, so "Недавно слушали" can show the playlist or album
// itself and not only its songs.

export type SourcePage = "playlist" | "smart" | "collection" | "files";

export interface PlaySource {
  page: SourcePage;
  /** playlist id or "likes", the smart kind, the collection id */
  id?: string;
  /** albums and service playlists: their name and cover, as the page loaded them */
  title?: string;
  cover?: string;
  /** "Альбом · SoundCloud" */
  kind?: string;
}

const SOURCE_PAGES: readonly string[] = ["playlist", "smart", "collection", "files"];

// pages that load their name from a service tell it here once they have it
const described = new Map<string, { title: string; cover?: string; kind?: string }>();

export function describeSource(id: string, title: string, cover?: string, kind?: string): void {
  described.set(id, { title, cover, kind });
}

export function sourceKey(s: PlaySource): string {
  return `${s.page}:${s.id ?? ""}`;
}

export function sameSource(a: PlaySource, b: PlaySource): boolean {
  return sourceKey(a) === sourceKey(b);
}

/** the page on screen when a queue starts, if it is one a queue can come from */
export function currentSource(): PlaySource | null {
  const nav = useNavigation.getState();
  if (!SOURCE_PAGES.includes(nav.page)) return null;
  const page = nav.page as SourcePage;
  const id = nav.playlistId;
  if (page === "collection") {
    if (!id) return null;
    const info = described.get(id);
    return { page, id, title: info?.title ?? nav.label, cover: info?.cover, kind: info?.kind };
  }
  return { page, id };
}
