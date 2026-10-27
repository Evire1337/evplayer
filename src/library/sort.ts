import type { Track } from "@/types";

export type SortKey = "custom" | "added" | "title" | "artist" | "duration" | "plays";
export type SortDir = "asc" | "desc";

export const SORT_LABEL: Record<SortKey, string> = {
  custom: "Свой порядок",
  added: "Дата добавления",
  title: "Название",
  artist: "Исполнитель",
  duration: "Длительность",
  plays: "Ваши прослушивания",
};

/** the direction a key starts with: newest, longest and most played first; names A→Я */
export const DEFAULT_DIR: Record<SortKey, SortDir> = { custom: "asc", added: "desc", title: "asc", artist: "asc", duration: "desc", plays: "desc" };

const collator = new Intl.Collator("ru", { sensitivity: "base", numeric: true });

export interface SortableRow {
  track: Track;
  addedAt: string;
}

export function sortRows<T extends SortableRow>(rows: T[], key: SortKey, dir: SortDir, plays: (id: string) => number = () => 0): T[] {
  if (key === "custom") return dir === "asc" ? rows : [...rows].reverse();
  const sign = dir === "asc" ? 1 : -1;
  const value = (r: T): string | number => {
    switch (key) {
      case "added":
        return r.addedAt;
      case "title":
        return r.track.title;
      case "artist":
        return r.track.artists[0]?.name ?? "";
      case "duration":
        return r.track.durationMs;
      case "plays":
        return plays(r.track.id);
    }
  };
  return rows
    .map((r, i) => ({ r, i, v: value(r) }))
    .sort((a, b) => {
      const c = typeof a.v === "number" && typeof b.v === "number" ? a.v - b.v : collator.compare(String(a.v), String(b.v));
      return c !== 0 ? c * sign : a.i - b.i;
    })
    .map((x) => x.r);
}
