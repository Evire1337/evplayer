import { create } from "zustand";

// One long import at a time (a link, a code, a Spotify or Yandex export), shown in a small
// panel with a progress bar and a cancel button. Steps report a line of text such as
// "Ищу песни: 12 из 80 (найдено 10)"; the numbers in it drive the bar, and the time of the
// last change tells whether the import is still moving or waiting on a slow service.

export interface ImportResult {
  ok: boolean;
  text: string;
}

interface ImportProgress {
  text: string | null;
  cancel: (() => void) | null;
  done: number | null;
  total: number | null;
  found: number | null;
  startedAt: number;
  /** when the text last changed: a long pause means the service is slow or stuck */
  changedAt: number;
  /** shown for a few seconds after the import ends */
  result: ImportResult | null;
  start: (text: string, cancel: () => void) => void;
  update: (text: string) => void;
  finish: (result?: ImportResult) => void;
}

/** "12 из 80", "найдено 10" in a step's text */
export function parseCounts(text: string): { done: number | null; total: number | null; found: number | null } {
  const of = /(\d+)\s+из\s+(\d+)/.exec(text);
  const found = /найдено\s+(\d+)/.exec(text);
  return { done: of ? Number(of[1]) : null, total: of ? Number(of[2]) : null, found: found ? Number(found[1]) : null };
}

const RESULT_SHOWN_MS = 4000;
let resultTimer: ReturnType<typeof setTimeout> | undefined;

export const useImportProgress = create<ImportProgress>((set) => ({
  text: null,
  cancel: null,
  done: null,
  total: null,
  found: null,
  startedAt: 0,
  changedAt: 0,
  result: null,
  start: (text, cancel) => {
    clearTimeout(resultTimer);
    const now = Date.now();
    set({ text, cancel, ...parseCounts(text), startedAt: now, changedAt: now, result: null });
  },
  update: (text) => set((s) => (s.text === null || s.text === text ? s : { text, ...parseCounts(text), changedAt: Date.now() })),
  finish: (result) => {
    clearTimeout(resultTimer);
    set({ text: null, cancel: null, result: result ?? null });
    if (result) resultTimer = setTimeout(() => set({ result: null }), RESULT_SHOWN_MS);
  },
}));
