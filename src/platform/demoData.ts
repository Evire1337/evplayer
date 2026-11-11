import type { Track } from "@/types";

// Sample data for looking at the whole design in a browser without a library of your own:
// open http://localhost:5173/?demo (once; `?demo=reset` fills it again). Pictures are drawn
// here, so nothing is fetched. Development only (platform/web.ts); a real install never has it.

const NAMES: Array<[string, string, number]> = [
  ["Metaroom", "EMBRYOGLYPH", 214],
  ["Madatracker", "Alexandria", 187],
  ["Земфира", "Хочешь?", 183],
  ["Playboi Carti", "Magnolia", 182],
  ["Mac DeMarco", "Chamber of Reflection", 232],
  ["Radiohead", "Everything In Its Right Place", 251],
  ["Макс Корж", "Жить в кайф", 175],
  ["The Weeknd", "Blinding Lights", 202],
  ["Boards of Canada", "Roygbiv", 150],
  ["Очень длинное имя исполнителя и ещё немного", "Очень длинное название песни, которое не помещается в одну строку списка", 305],
  ["Lana Del Rey", "Summertime Sadness", 265],
  ["Kendrick Lamar", "Not Like Us", 275],
  ["Daft Punk", "Veridis Quo", 345],
  ["Billie Eilish", "bad guy", 194],
];

const cover = (seed: number): string => {
  const hue = (seed * 47) % 360;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="400" height="400"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${hue},70%,55%)"/><stop offset="1" stop-color="hsl(${(hue + 70) % 360},75%,25%)"/></linearGradient></defs><rect width="400" height="400" fill="url(#g)"/><circle cx="${120 + (seed % 5) * 40}" cy="${140 + (seed % 3) * 50}" r="${70 + (seed % 4) * 15}" fill="hsla(${(hue + 160) % 360},80%,70%,0.35)"/></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
};

export const demoTracks: Track[] = NAMES.map(([artist, title, seconds], i) => ({
  id: `sc:${9_000_000 + i}`,
  source: "soundcloud",
  title,
  artists: [{ id: `scu:${i}`, name: artist }],
  durationMs: seconds * 1000,
  artwork: cover(i + 1),
  permalink: `https://soundcloud.com/demo/track-${i}`,
}));

const ago = (minutes: number) => new Date(Date.now() - minutes * 60_000).toISOString();

const PLAYLISTS: Array<[string, number[]]> = [
  ["Любимое", [0, 1, 2, 3, 4, 5]],
  ["Для дороги", [6, 7, 8, 9]],
  ["Очень длинное название плейлиста, которое точно не поместится в одну строку карточки", [10, 11, 12]],
  ["Phonk", [3, 13]],
  ["Утро", [4, 5, 8]],
  ["Пустой", []],
];

export function demoStorage(): Record<string, unknown> {
  const playlists = PLAYLISTS.map(([title, ids], p) => ({
    id: `pl_demo${p}`,
    title,
    createdAt: ago(60 * 24 * (p + 1)),
    updatedAt: ago(60 * (p + 1)),
    items: ids.map((i, n) => ({ id: `it_${p}_${n}`, track: demoTracks[i], addedAt: ago(30 * (n + 1)) })),
  }));
  return {
    "evplayer.library.v1": { state: { likes: demoTracks.slice(0, 9).map((track, i) => ({ track, addedAt: ago(10 * (i + 1)) })), playlists }, version: 0 },
    "evplayer.history.v1": {
      state: {
        entries: demoTracks.map((track, i) => ({ track, playedAt: ago(5 + i * 9), ...(i % 3 === 0 ? { from: "playlist:pl_demo0" } : {}) })),
        sources: [0, 1, 2].map((p) => ({ source: { page: "playlist", id: `pl_demo${p}` }, playedAt: ago(20 + p * 40) })),
      },
      version: 0,
    },
  };
}

/** fills the storage when the address says `?demo` (and it is empty, or says `?demo=reset`) */
export function seedDemo(): void {
  try {
    const flag = new URLSearchParams(location.search).get("demo");
    if (flag === null) return;
    const reset = flag === "reset";
    for (const [key, value] of Object.entries(demoStorage())) {
      if (reset || localStorage.getItem(key) === null) localStorage.setItem(key, JSON.stringify(value));
    }
  } catch {
    // no storage: the page simply starts empty
  }
}
