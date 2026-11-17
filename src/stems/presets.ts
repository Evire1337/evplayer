import { FLAT_STEMS, MAX_STEM_LEVEL, PARTS, type Part, type StemFaders } from "./model";

// The user's own quick settings of "Дорожки", the way the equalizer has its own presets: a name,
// the levels of the four tracks and which of them are switched off. They can be renamed,
// reordered, updated in place and passed on as a short code.

export interface CustomStemPreset {
  id: string;
  name: string;
  faders: StemFaders;
  muted: Part[];
}

const NAME_MAX = 32;

const clean = (name: string, fallback: string) => name.trim().slice(0, NAME_MAX) || fallback;

export function newPreset(name: string, faders: StemFaders, muted: readonly Part[], now = Date.now()): CustomStemPreset {
  return { id: `sp_${now.toString(36)}`, name: clean(name, "Пресет"), faders: { ...faders }, muted: [...muted] };
}

export const addPreset = (list: CustomStemPreset[], preset: CustomStemPreset): CustomStemPreset[] => [...list, preset];

export const renamePreset = (list: CustomStemPreset[], id: string, name: string): CustomStemPreset[] =>
  list.map((p) => (p.id === id ? { ...p, name: clean(name, p.name) } : p));

export const overwritePreset = (list: CustomStemPreset[], id: string, faders: StemFaders, muted: readonly Part[]): CustomStemPreset[] =>
  list.map((p) => (p.id === id ? { ...p, faders: { ...faders }, muted: [...muted] } : p));

export const removePreset = (list: CustomStemPreset[], id: string): CustomStemPreset[] => list.filter((p) => p.id !== id);

export function movePreset(list: CustomStemPreset[], from: number, to: number): CustomStemPreset[] {
  const next = [...list];
  const [item] = next.splice(from, 1);
  if (!item) return list;
  next.splice(Math.max(0, Math.min(next.length, to)), 0, item);
  return next;
}

export const samePresetLevels = (p: Pick<CustomStemPreset, "faders" | "muted">, faders: StemFaders, muted: readonly Part[]): boolean =>
  PARTS.every((k) => Math.abs(p.faders[k] - faders[k]) < 0.001) && p.muted.length === muted.length && p.muted.every((m) => muted.includes(m));

// ---- a preset as a line of text --------------------------------------------------------

const PREFIX = "EVS1.";

export function encodeStemPreset(p: CustomStemPreset): string {
  const wire = { n: p.name, f: PARTS.map((k) => Math.round(p.faders[k] * 100)), m: p.muted.map((m) => PARTS.indexOf(m)) };
  const bytes = new TextEncoder().encode(JSON.stringify(wire));
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return PREFIX + btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** the preset in a pasted code, or null when it is not one; extra spaces and line breaks are fine */
export function decodeStemPreset(text: string, now = Date.now()): CustomStemPreset | null {
  const code = text.replace(/[\s​-‍﻿]/g, "");
  if (!code.startsWith(PREFIX)) return null;
  try {
    const body = code.slice(PREFIX.length).replace(/=+$/, "").replace(/-/g, "+").replace(/_/g, "/");
    const bin = atob(body + "=".repeat((4 - (body.length % 4)) % 4));
    const wire = JSON.parse(new TextDecoder().decode(Uint8Array.from(bin, (c) => c.charCodeAt(0)))) as { n?: unknown; f?: unknown; m?: unknown };
    if (typeof wire.n !== "string" || !Array.isArray(wire.f) || wire.f.length !== PARTS.length || !Array.isArray(wire.m)) return null;
    const faders = { ...FLAT_STEMS };
    for (const [i, k] of PARTS.entries()) {
      const v = Number(wire.f[i]);
      if (!Number.isFinite(v)) return null;
      faders[k] = Math.min(MAX_STEM_LEVEL, Math.max(0, v / 100));
    }
    const muted = wire.m.map((i) => PARTS[Number(i)]).filter((p): p is Part => Boolean(p));
    return newPreset(wire.n, faders, muted, now);
  } catch {
    return null;
  }
}
