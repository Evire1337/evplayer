import { keeps } from "@/settings/store";
// Volume levelling: SoundCloud uploads and YouTube videos are mastered at wildly different
// levels. This follows how loud the current song is (a slow moving average of its power,
// so single drum hits don't pump) and turns it up or down toward one target level. The
// estimate is remembered per track, so a song you have heard before starts at its level.

export const TARGET_DB = -18;
export const MAX_BOOST_DB = 8;
export const MAX_CUT_DB = -12;
const SILENCE_DB = -55;
const STORAGE_KEY = "evplayer.loudness.v1";
const MAX_REMEMBERED = 3000;

export class LoudnessLeveler {
  private avgPower: number | null = null;
  private heardMs = 0;

  constructor(initialDb?: number) {
    if (initialDb !== undefined) {
      this.avgPower = Math.pow(10, initialDb / 10);
      this.heardMs = 30_000;
    }
  }

  /** feed one measurement (mean square of the samples) taken `dtMs` after the last one */
  update(power: number, dtMs: number): void {
    const db = 10 * Math.log10(Math.max(power, 1e-12));
    if (db < SILENCE_DB) return; // pauses and silent intros say nothing about the level
    // adapt fast during the first seconds of an unknown song, slowly afterwards
    const tau = this.heardMs < 6000 ? 1500 : 8000;
    const alpha = 1 - Math.exp(-dtMs / tau);
    this.avgPower = this.avgPower === null ? power : this.avgPower + alpha * (power - this.avgPower);
    this.heardMs += dtMs;
  }

  loudnessDb(): number | null {
    return this.avgPower === null ? null : 10 * Math.log10(this.avgPower);
  }

  gainDb(): number {
    const loudness = this.loudnessDb();
    if (loudness === null) return 0;
    return Math.min(MAX_BOOST_DB, Math.max(MAX_CUT_DB, TARGET_DB - loudness));
  }

  isSettled(): boolean {
    return this.heardMs >= 20_000;
  }
}

let remembered: Record<string, number> | null = null;

function memory(): Record<string, number> {
  if (remembered) return remembered;
  try {
    remembered = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as Record<string, number>;
  } catch {
    remembered = {};
  }
  return remembered;
}

export function rememberedLoudness(trackId: string): number | undefined {
  const value = memory()[trackId];
  return typeof value === "number" ? value : undefined;
}

export function rememberLoudness(trackId: string, db: number): void {
  if (!keeps("loudness")) return;
  const store = memory();
  delete store[trackId];
  store[trackId] = Math.round(db * 10) / 10;
  const keys = Object.keys(store);
  for (let i = 0; i < keys.length - MAX_REMEMBERED; i++) delete store[keys[i]];
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
  } catch {
    // not important
  }
}
