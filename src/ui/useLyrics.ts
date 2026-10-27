import { useEffect, useState } from "react";
import type { Track } from "@/types";
import { findLyrics, readings } from "@/services/lyrics/lrclib";
import { findGeniusLyrics } from "@/services/lyrics/genius";
import { parseLrc, type LyricLine } from "@/services/lyrics/lrc";
import { useSettings } from "@/settings/store";

export type LyricsState =
  | { status: "idle" | "loading" | "missing" | "error" }
  | { status: "ready"; synced: boolean; lines: LyricLine[]; source: string };

// a small cache: lyrics of the last songs, so going back and forth is instant
const cache = new Map<string, LyricsState>();
const CACHE_LIMIT = 40;

function remember(id: string, state: LyricsState): void {
  cache.delete(id);
  cache.set(id, state);
  if (cache.size > CACHE_LIMIT) cache.delete(cache.keys().next().value!);
}

async function load(track: Track, signal: AbortSignal): Promise<LyricsState> {
  const found = await findLyrics(track, signal).catch(() => null);
  if (found?.text) return { status: "ready", source: found.source, ...parseLrc(found.text) };
  if (useSettings.getState().geniusFallback) {
    for (const r of readings(track)) {
      const text = await findGeniusLyrics(r, signal).catch(() => undefined);
      if (text) return { status: "ready", source: "Genius", ...parseLrc(text) };
    }
  }
  return { status: "missing" };
}

export function useLyrics(track: Track | undefined): LyricsState {
  const [state, setState] = useState<LyricsState>({ status: "idle" });
  useEffect(() => {
    if (!track) {
      setState({ status: "idle" });
      return;
    }
    const hit = cache.get(track.id);
    if (hit) {
      setState(hit);
      return;
    }
    const controller = new AbortController();
    setState({ status: "loading" });
    load(track, controller.signal)
      .then((next) => {
        remember(track.id, next);
        if (!controller.signal.aborted) setState(next);
      })
      .catch(() => !controller.signal.aborted && setState({ status: "error" }));
    return () => controller.abort();
  }, [track]);
  return state;
}

export function clearLyricsCache(): void {
  cache.clear();
}
