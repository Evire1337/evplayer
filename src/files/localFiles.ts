import { create } from "zustand";
import type { Track } from "@/types";
import type { LocalTrackRecord } from "../../electron/local-files";
import { toast } from "@/ui/toasts";

// Your own audio files. The main process keeps the index; this mirrors it for the page.

export function recordToTrack(r: LocalTrackRecord): Track {
  return {
    id: r.id,
    source: "local",
    title: r.title,
    artists: (r.artists.length ? r.artists : ["Неизвестный исполнитель"]).map((name) => ({ id: `local-artist:${name.toLowerCase()}`, name })),
    durationMs: r.durationMs,
    artwork: r.hasCover ? `evplayer-media://cover/${encodeURIComponent(r.id)}?v=${encodeURIComponent(r.addedAt)}${r.edited ? "e" : ""}` : undefined,
  };
}

export function formatLabel(r: LocalTrackRecord): string {
  const parts: string[] = [];
  if (r.codec) parts.push(r.codec.replace(/^MPEG 1 Layer 3$/i, "MP3").toUpperCase());
  if (r.lossless && r.bitsPerSample && r.sampleRate) parts.push(`${r.bitsPerSample} бит / ${Math.round(r.sampleRate / 100) / 10} кГц`);
  else if (r.bitrate) parts.push(`${Math.round(r.bitrate / 1000)} кбит/с`);
  return parts.join(" · ");
}

interface LocalFilesState {
  records: LocalTrackRecord[];
  folders: string[];
  loaded: boolean;
  refresh: () => Promise<void>;
  pick: (kind: "files" | "folder") => Promise<void>;
  drop: (files: File[]) => Promise<void>;
  remove: (ids: string[]) => Promise<void>;
  unwatch: (folder: string) => Promise<void>;
}

const bridge = () => window.evplayer?.localFiles;

function report(result: { added: unknown[]; skipped: number }): void {
  if (!result.added.length && !result.skipped) return;
  toast(result.added.length ? `Добавлено файлов: ${result.added.length}${result.skipped ? `, пропущено: ${result.skipped}` : ""}` : "В выбранном нет аудиофайлов", result.added.length ? "ok" : "error");
}

export const useLocalFiles = create<LocalFilesState>((set, get) => ({
  records: [],
  folders: [],
  loaded: false,
  refresh: async () => {
    const api = bridge();
    if (!api) return set({ loaded: true });
    const [records, folders] = await Promise.all([api.list(), api.watchedFolders()]);
    set({ records, folders, loaded: true });
  },
  pick: async (kind) => {
    const api = bridge();
    if (!api) return;
    report(await api.pick(kind));
    await get().refresh();
  },
  drop: async (files) => {
    const api = bridge();
    if (!api || !files.length) return;
    report(await api.addDropped(files));
    await get().refresh();
  },
  remove: async (ids) => {
    await bridge()?.remove(ids);
    await get().refresh();
  },
  unwatch: async (folder) => {
    await bridge()?.unwatchFolder(folder);
    await get().refresh();
  },
}));

if (typeof window !== "undefined") {
  window.evplayer?.localFiles.onChanged(() => void useLocalFiles.getState().refresh());
}
