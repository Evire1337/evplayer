// Parametric equalizer model. Bands map one to one onto Web Audio BiquadFilterNodes, and the
// response math below follows the same formulas (Audio EQ Cookbook, the shelf slope fixed at
// S = 1 as in Web Audio) so the curve drawn on screen is the curve you hear.

export type BandType = "lowshelf" | "peaking" | "highshelf";

export interface EqBand {
  id: string;
  type: BandType;
  frequency: number;
  gain: number;
  q: number;
  enabled: boolean;
}

export const MIN_FREQ = 20;
export const MAX_FREQ = 20000;
export const MAX_GAIN = 12;
export const MIN_Q = 0.3;
export const MAX_Q = 8;

export const BAND_COLORS = ["#f5c242", "#ff8a4c", "#38bdf8", "#a78bfa", "#f472b6", "#34d399"];

export function defaultBands(): EqBand[] {
  return [
    { id: "b1", type: "lowshelf", frequency: 60, gain: 0, q: 0.71, enabled: true },
    { id: "b2", type: "peaking", frequency: 150, gain: 0, q: 1, enabled: true },
    { id: "b3", type: "peaking", frequency: 500, gain: 0, q: 1, enabled: true },
    { id: "b4", type: "peaking", frequency: 1500, gain: 0, q: 1, enabled: true },
    { id: "b5", type: "peaking", frequency: 4000, gain: 0, q: 1, enabled: true },
    { id: "b6", type: "highshelf", frequency: 10000, gain: 0, q: 0.71, enabled: true },
  ];
}

export type PresetId = "flat" | "rich" | "bass" | "vocal" | "warm" | "bright";

const PRESET_GAINS: Record<PresetId, number[]> = {
  flat: [0, 0, 0, 0, 0, 0],
  // the gentle "smile" most people mean by a fuller sound: weight down low, air up top,
  // a little less boxiness in the low mids
  rich: [4, 1.5, -1.5, 0, 2, 3.5],
  bass: [6, 2.5, 0, 0, 0, 0],
  vocal: [-1, -2, 0, 3, 2.5, 0.5],
  warm: [3, 1, 1.5, 0, -1, -2],
  bright: [0, 0, -0.5, 1, 2.5, 5],
};

export const PRESET_IDS = Object.keys(PRESET_GAINS) as PresetId[];

export function presetBands(id: PresetId): EqBand[] {
  return defaultBands().map((band, i) => ({ ...band, gain: PRESET_GAINS[id][i] ?? 0 }));
}

export const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

interface Coefficients {
  b0: number;
  b1: number;
  b2: number;
  a0: number;
  a1: number;
  a2: number;
}

function coefficients(band: EqBand, sampleRate: number): Coefficients {
  const A = Math.pow(10, band.gain / 40);
  const w0 = (2 * Math.PI * clamp(band.frequency, 1, sampleRate / 2 - 1)) / sampleRate;
  const cos = Math.cos(w0);
  const sin = Math.sin(w0);
  if (band.type === "peaking") {
    const alpha = sin / (2 * Math.max(band.q, 1e-4));
    return { b0: 1 + alpha * A, b1: -2 * cos, b2: 1 - alpha * A, a0: 1 + alpha / A, a1: -2 * cos, a2: 1 - alpha / A };
  }
  const alpha = (sin / 2) * Math.SQRT2;
  const sq = 2 * Math.sqrt(A) * alpha;
  if (band.type === "lowshelf") {
    return {
      b0: A * (A + 1 - (A - 1) * cos + sq),
      b1: 2 * A * (A - 1 - (A + 1) * cos),
      b2: A * (A + 1 - (A - 1) * cos - sq),
      a0: A + 1 + (A - 1) * cos + sq,
      a1: -2 * (A - 1 + (A + 1) * cos),
      a2: A + 1 + (A - 1) * cos - sq,
    };
  }
  return {
    b0: A * (A + 1 + (A - 1) * cos + sq),
    b1: -2 * A * (A - 1 + (A + 1) * cos),
    b2: A * (A + 1 + (A - 1) * cos - sq),
    a0: A + 1 - (A - 1) * cos + sq,
    a1: 2 * (A - 1 - (A + 1) * cos),
    a2: A + 1 - (A - 1) * cos - sq,
  };
}

/** gain in dB this band applies at `frequency` */
export function bandResponseDb(band: EqBand, frequency: number, sampleRate = 48000): number {
  if (!band.enabled || band.gain === 0) return 0;
  const c = coefficients(band, sampleRate);
  const w = (2 * Math.PI * frequency) / sampleRate;
  const cos1 = Math.cos(w);
  const sin1 = Math.sin(w);
  const cos2 = Math.cos(2 * w);
  const sin2 = Math.sin(2 * w);
  const nr = c.b0 + c.b1 * cos1 + c.b2 * cos2;
  const ni = -(c.b1 * sin1 + c.b2 * sin2);
  const dr = c.a0 + c.a1 * cos1 + c.a2 * cos2;
  const di = -(c.a1 * sin1 + c.a2 * sin2);
  const magnitude = Math.sqrt((nr * nr + ni * ni) / (dr * dr + di * di));
  return 20 * Math.log10(magnitude);
}

export function totalResponseDb(bands: EqBand[], frequency: number, sampleRate = 48000): number {
  return bands.reduce((sum, band) => sum + bandResponseDb(band, frequency, sampleRate), 0);
}

/** how much the curve boosts at its loudest point, used to keep the output from clipping */
export function peakBoostDb(bands: EqBand[]): number {
  let peak = 0;
  for (let i = 0; i <= 120; i++) {
    const f = MIN_FREQ * Math.pow(MAX_FREQ / MIN_FREQ, i / 120);
    peak = Math.max(peak, totalResponseDb(bands, f));
  }
  return peak;
}

// log frequency axis helpers for the editor
export function freqToX(frequency: number, width: number): number {
  return (Math.log(frequency / MIN_FREQ) / Math.log(MAX_FREQ / MIN_FREQ)) * width;
}

export function xToFreq(x: number, width: number): number {
  return MIN_FREQ * Math.pow(MAX_FREQ / MIN_FREQ, clamp(x / width, 0, 1));
}

export function formatFrequency(frequency: number): string {
  return frequency >= 1000 ? `${(frequency / 1000).toFixed(frequency >= 10000 ? 0 : 1)} kHz` : `${Math.round(frequency)} Hz`;
}
