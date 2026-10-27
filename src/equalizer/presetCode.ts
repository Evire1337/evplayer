import { clamp, defaultBands, MAX_FREQ, MAX_GAIN, MAX_Q, MIN_FREQ, MIN_Q, type BandType, type EqBand } from "./eqModel";
import type { CustomPreset } from "./eqStore";

// EVQ1.<base64url json>: an equalizer preset that can be pasted to a friend
const TYPES: BandType[] = ["lowshelf", "peaking", "highshelf"];

function toBase64Url(text: string): string {
  const bytes = new TextEncoder().encode(text);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function fromBase64Url(text: string): string {
  const normalized = text.replace(/\s+/g, "").replace(/-/g, "+").replace(/_/g, "/");
  const bin = atob(normalized + "=".repeat((4 - (normalized.length % 4)) % 4));
  return new TextDecoder().decode(Uint8Array.from(bin, (c) => c.charCodeAt(0)));
}

export function encodePresetCode(preset: Pick<CustomPreset, "name" | "bands" | "preamp">): string {
  const wire = {
    v: 1,
    n: preset.name,
    p: preset.preamp,
    b: preset.bands.map((b) => [TYPES.indexOf(b.type), Math.round(b.frequency), b.gain, Math.round(b.q * 100) / 100, b.enabled ? 1 : 0]),
  };
  return `EVQ1.${toBase64Url(JSON.stringify(wire))}`;
}

export function decodePresetCode(code: string): Omit<CustomPreset, "id"> {
  const match = /^\s*EVQ1\.([A-Za-z0-9_\-\s]+)\s*$/.exec(code);
  if (!match) throw new Error("not a preset code");
  const wire = JSON.parse(fromBase64Url(match[1])) as { v: number; n: string; p: number; b: number[][] };
  if (wire.v !== 1 || !Array.isArray(wire.b)) throw new Error("unknown preset format");
  const base = defaultBands();
  const bands: EqBand[] = base.map((band, i) => {
    const row = wire.b[i];
    if (!Array.isArray(row)) return band;
    return {
      ...band,
      type: TYPES[row[0]] ?? band.type,
      frequency: clamp(Number(row[1]) || band.frequency, MIN_FREQ, MAX_FREQ),
      gain: clamp(Number(row[2]) || 0, -MAX_GAIN, MAX_GAIN),
      q: clamp(Number(row[3]) || band.q, MIN_Q, MAX_Q),
      enabled: row[4] !== 0,
    };
  });
  return { name: String(wire.n || "Preset").slice(0, 32), preamp: clamp(Number(wire.p) || 0, -12, 6), bands };
}
