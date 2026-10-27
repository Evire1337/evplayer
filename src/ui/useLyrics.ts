import { useEffect, useState } from "react";
import type { Track } from "@/types";
import { findLyrics } from "@/services/lyrics/lrclib";
import { parseLrc, type LyricLine } from "@/services/lyrics/lrc";

export type LyricsState =
  | { status: "idle" | "loading" | "missing" | "error" }
  | { status: "ready"; synced: boolean; lines: LyricLine[]; source: string };

const cache = new Map<string, LyricsState>();

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
    findLyrics(track, controller.signal)
      .then((found) => {
        const next: LyricsState = found ? { status: "ready", source: found.source, ...parseLrc(found.text) } : { status: "missing" };
        cache.set(track.id, next);
        if (!controller.signal.aborted) setState(next);
      })
      .catch(() => !controller.signal.aborted && setState({ status: "error" }));
    return () => controller.abort();
  }, [track]);
  return state;
}
