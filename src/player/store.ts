import { create } from "zustand";
import type { Track } from "@/types";

export type PlayerStatus = "idle" | "loading" | "playing" | "paused" | "error";
export type RepeatMode = "off" | "all" | "one";

export interface PlayerState {
  queue: Track[];
  index: number;
  status: PlayerStatus;
  positionMs: number;
  durationMs: number;
  volume: number;
  muted: boolean;
  repeat: RepeatMode;
  shuffle: boolean;
  error?: string;
  /** the original did not play: a copy of the song from this service is playing */
  route?: "soundcloud" | "youtube" | "local";
}

const VOLUME_KEY = "evplayer.volume";

function savedVolume(): number {
  try {
    const v = Number(localStorage.getItem(VOLUME_KEY));
    return Number.isFinite(v) && v > 0 && v <= 1 ? v : 0.8;
  } catch {
    return 0.8;
  }
}

export const usePlayer = create<PlayerState>(() => ({
  queue: [],
  index: -1,
  status: "idle",
  positionMs: 0,
  durationMs: 0,
  volume: savedVolume(),
  muted: false,
  repeat: "off",
  shuffle: false,
}));

usePlayer.subscribe((state, prev) => {
  if (state.volume !== prev.volume) {
    try {
      localStorage.setItem(VOLUME_KEY, String(state.volume));
    } catch {
      // not important
    }
  }
});

export function currentTrack(state: PlayerState = usePlayer.getState()): Track | undefined {
  return state.queue[state.index];
}

/** index of the next track, or -1 at the end of the queue */
export function nextIndex(state: PlayerState, manual: boolean): number {
  const { queue, index, repeat } = state;
  if (queue.length === 0) return -1;
  if (repeat === "one" && !manual) return index;
  if (index + 1 < queue.length) return index + 1;
  // at the end: start over only with "repeat all"; otherwise the player decides
  // (similar songs when autoplay is on)
  return repeat === "all" ? 0 : -1;
}

export function prevIndex(state: PlayerState): number {
  if (state.queue.length === 0) return -1;
  return state.index > 0 ? state.index - 1 : state.repeat === "all" ? state.queue.length - 1 : 0;
}

/** Fisher-Yates, keeping the given track first */
export function shuffledQueue(queue: Track[], firstId?: string): Track[] {
  const rest = queue.filter((t) => t.id !== firstId);
  for (let i = rest.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [rest[i], rest[j]] = [rest[j], rest[i]];
  }
  const first = queue.find((t) => t.id === firstId);
  return first ? [first, ...rest] : rest;
}
