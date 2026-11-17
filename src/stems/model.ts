// "Дорожки" with the neural splitter: a song becomes four tracks (stems.rs on the desktop side),
// and these are the names and the pure rules around them: what is heard from the faders, the
// muted ones and the solo.

export type Part = "vocals" | "drums" | "bass" | "other";
/** the order of the files and of the networks in stems.rs */
export const PARTS: readonly Part[] = ["vocals", "drums", "bass", "other"];

export const PART_TITLE: Record<Part, string> = { vocals: "Вокал", drums: "Барабаны", bass: "Бас", other: "Остальное" };
export const PART_HINT: Record<Part, string> = {
  vocals: "голос и подпевки",
  drums: "ударные: бочка, малый, тарелки",
  bass: "бас-гитара и низкие синтезаторы",
  other: "гитары, клавиши, струнные и всё прочее",
};

/** how far a fader can go (1 = as recorded) */
export const MAX_STEM_LEVEL = 1.5;

export type StemFaders = Record<Part, number>;
export const FLAT_STEMS: StemFaders = { vocals: 1, drums: 1, bass: 1, other: 1 };

const clamp = (v: number): number => (Number.isFinite(v) ? Math.min(MAX_STEM_LEVEL, Math.max(0, v)) : 1);

/** the level each track plays at: the fader, zero if muted, and only the solo one while there is a solo */
export function stemGains(faders: StemFaders, muted: readonly Part[], solo: Part | null): number[] {
  return PARTS.map((p) => {
    if (muted.includes(p)) return 0;
    if (solo !== null && solo !== p) return 0;
    return clamp(faders[p]);
  });
}

export interface StemPreset {
  name: string;
  faders: StemFaders;
  muted: Part[];
}

export const STEM_PRESETS: readonly StemPreset[] = [
  { name: "Без вокала", faders: FLAT_STEMS, muted: ["vocals"] },
  { name: "Только вокал", faders: FLAT_STEMS, muted: ["drums", "bass", "other"] },
  { name: "Без барабанов", faders: FLAT_STEMS, muted: ["drums"] },
  { name: "Без баса", faders: FLAT_STEMS, muted: ["bass"] },
  { name: "Только ритм", faders: FLAT_STEMS, muted: ["vocals", "other"] },
];

export const samePreset = (p: StemPreset, faders: StemFaders, muted: readonly Part[], solo: Part | null): boolean =>
  solo === null && PARTS.every((k) => Math.abs(p.faders[k] - faders[k]) < 0.001) && p.muted.length === muted.length && p.muted.every((m) => muted.includes(m));

// ---- the key of a song ----------------------------------------------------------------

/** a short key for a song: the same song always gets the same one, so its tracks are found again */
export async function stemKey(trackId: string, durationMs: number): Promise<string> {
  const bytes = new TextEncoder().encode(`evp1|${trackId}|${Math.round(durationMs / 1000)}`);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)].slice(0, 16).map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** a song longer than this is left alone: it would take too long and too much memory */
export const MAX_STEM_SECONDS = 12 * 60;
