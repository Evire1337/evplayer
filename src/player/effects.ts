import { create } from "zustand";
import { ratioToSemitones } from "@/audio/pitch";

export const SPEED_PRESETS = [0.8, 0.9, 1, 1.15, 1.25, 1.35] as const;
export const MIN_SPEED = 0.5;
export const MAX_SPEED = 2;
export const MAX_PITCH = 12;

export interface EffectsState {
  speed: number;
  /** independent pitch in semitones; ignored while linked */
  pitch: number;
  /** speed and pitch move together, like a record played faster */
  linked: boolean;
  /** keep these settings when the next track starts */
  keep: boolean;
  reverb: boolean;
  reverbAmount: number;
}

const KEY = "evplayer.effects";
export const DEFAULT_EFFECTS: EffectsState = { speed: 1, pitch: 0, linked: false, keep: false, reverb: false, reverbAmount: 0.5 };

function load(): EffectsState {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "null") as Partial<EffectsState> | null;
    return raw?.keep ? { ...DEFAULT_EFFECTS, ...raw } : DEFAULT_EFFECTS;
  } catch {
    return DEFAULT_EFFECTS;
  }
}

export const useEffects = create<EffectsState>(() => load());

useEffects.subscribe((state) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(state));
  } catch {
    // not important
  }
});

export const clampSpeed = (v: number) => Math.round(Math.min(MAX_SPEED, Math.max(MIN_SPEED, v)) * 100) / 100;
export const clampPitch = (v: number) => Math.round(Math.min(MAX_PITCH, Math.max(-MAX_PITCH, v)) * 10) / 10;

/** what the pitch actually is: the speed's own shift when linked */
export function effectivePitch(state: EffectsState): number {
  return state.linked ? ratioToSemitones(state.speed) : state.pitch;
}

/** a new track starts: forget the settings unless asked to keep them */
export function effectsForNextTrack(state: EffectsState): EffectsState {
  return state.keep ? state : { ...DEFAULT_EFFECTS, keep: false };
}
