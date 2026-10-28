import { useEffect, useState } from "react";
import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { activeLineIndex, type LyricLine } from "@/services/lyrics/lrc";

// When a lyric line lights up.
//
// The song's position used to come from the player's progress ticks (about four a second),
// with a fixed quarter second added, so a line could light up to a quarter second late or
// early, and more on systems with a long sound path. Here the line is worked out from the
// audio element's exact time, minus the delay of the sound path (what you hear is that much
// behind), and the switch to the next line is timed for the moment it starts.
//
// Synced lyrics also come from a database and can be timed for a slightly different
// version of the song (a second of silence more at the start): the shift of each song can
// be nudged by hand and is remembered.

/** a line shows a moment before it is sung, so you read it as the singer starts it */
const LEAD_MS = 120;
export const OFFSET_STEP_MS = 250;
const MAX_OFFSET_MS = 10_000;
const REMEMBERED = 300;

interface Offsets {
  /** per song: + shows the text earlier, − later */
  byTrack: Record<string, number>;
  nudge: (trackId: string, deltaMs: number) => void;
  reset: (trackId: string) => void;
}

export const useLyricsOffsets = create<Offsets>()(
  persist(
    (set) => ({
      byTrack: {},
      nudge: (trackId, deltaMs) =>
        set((s) => {
          const value = Math.max(-MAX_OFFSET_MS, Math.min(MAX_OFFSET_MS, (s.byTrack[trackId] ?? 0) + deltaMs));
          const rest = Object.entries(s.byTrack).filter(([id]) => id !== trackId);
          const kept = rest.slice(Math.max(0, rest.length - REMEMBERED + 1));
          return { byTrack: Object.fromEntries(value ? [...kept, [trackId, value]] : kept) };
        }),
      reset: (trackId) =>
        set((s) => {
          const { [trackId]: _drop, ...rest } = s.byTrack;
          return { byTrack: rest };
        }),
    }),
    { name: "evplayer.lyricsOffset.v1", storage: createJSONStorage(() => localStorage) },
  ),
);

export const useLyricsOffset = (trackId: string | undefined) => useLyricsOffsets((s) => (trackId ? (s.byTrack[trackId] ?? 0) : 0));

// Some systems move `currentTime` in steps (a fifth of a second and more) rather than
// smoothly. It is exact at the moment it changes, so the time since that change is added.
let anchor = { audio: null as HTMLAudioElement | null, time: -1, at: 0 };
const MAX_EXTRAPOLATE_MS = 400;

function songTimeMs(audio: HTMLAudioElement): number {
  const now = performance.now();
  const time = audio.currentTime;
  if (anchor.audio !== audio || anchor.time !== time) {
    anchor = { audio, time, at: now };
    return time * 1000;
  }
  // still or waiting for data: no guessing ahead
  if (audio.paused || audio.readyState < 3) return time * 1000;
  return time * 1000 + Math.min(MAX_EXTRAPOLATE_MS, (now - anchor.at) * (audio.playbackRate || 1));
}

/** the position lyrics are matched against, in ms */
export function lyricsPosition(offsetMs: number): number {
  const audio = player.audio;
  const raw = audio.getAttribute("src") || audio.currentTime > 0 ? songTimeMs(audio) : usePlayer.getState().positionMs;
  return raw - player.outputLatencyMs + LEAD_MS + offsetMs;
}

/** the line being sung now; re-renders only when it changes */
export function useActiveLine(lines: LyricLine[] | null, trackId: string | undefined): number {
  const offset = useLyricsOffset(trackId);
  const [index, setIndex] = useState(-1);
  useEffect(() => {
    if (!lines?.length) {
      setIndex(-1);
      return;
    }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const tick = () => {
      clearTimeout(timer);
      const pos = lyricsPosition(offset);
      const i = activeLineIndex(lines, pos);
      setIndex(i);
      const audio = player.audio;
      const next = lines[i + 1];
      // wake up right when the next line starts (the progress ticks re-sync it anyway)
      if (next && !audio.paused) timer = setTimeout(tick, Math.max(15, (next.timeMs - pos) / (audio.playbackRate || 1) + 5));
    };
    tick();
    const off = usePlayer.subscribe((s, prev) => {
      if (s.positionMs !== prev.positionMs || s.status !== prev.status || s.index !== prev.index) tick();
    });
    return () => {
      clearTimeout(timer);
      off();
    };
  }, [lines, offset]);
  return index;
}
