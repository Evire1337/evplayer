import type { Lang } from "./lang";
import { EN } from "./en";
import { EN_PATTERNS } from "./patterns";

/** English: "1 track", "5 tracks" */
export const plural = (n: number, one: string, many: string): string => `${n} ${n === 1 ? one : many}`;

/** one piece of text (no surrounding spaces) in the language; the same text when there is no translation */
function piece(text: string): string {
  const exact = EN[text];
  if (exact !== undefined) return exact;
  for (const [pattern, make] of EN_PATTERNS) {
    const m = pattern.exec(text);
    if (m) return make(m, piece);
  }
  return text;
}

/**
 * A text of the interface in the language. Spaces around it stay; "A · B" is two pieces (the
 * dot is how the app joins a name and a count); a text nobody has translated stays Russian.
 */
export function translate(text: string, lang: Lang): string {
  if (lang === "ru" || !/[А-Яа-яЁё]/.test(text)) return text;
  const lead = /^\s*/.exec(text)![0];
  const trail = /\s*$/.exec(text)![0];
  const core = text.trim();
  if (!core) return text;
  let out = piece(core);
  if (out === core && core.includes(" · ")) out = core.split(" · ").map(piece).join(" · ");
  return lead + out + trail;
}
