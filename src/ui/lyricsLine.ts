import { create } from "zustand";

/** the lyric line being sung right now, for the mini player */
export const useLyricsLine = create<{ line: string | null }>(() => ({ line: null }));
