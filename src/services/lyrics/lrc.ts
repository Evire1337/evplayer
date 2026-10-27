export interface LyricLine {
  timeMs: number;
  text: string;
}

const STAMP = /\[(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;

/** "[01:23.45] words" -> timed lines, sorted; untimed text gives one line per row with no time */
export function parseLrc(source: string): { synced: boolean; lines: LyricLine[] } {
  const lines: LyricLine[] = [];
  let timed = false;
  for (const raw of source.split(/\r?\n/)) {
    const stamps = [...raw.matchAll(STAMP)];
    const text = raw.replace(STAMP, "").trim();
    if (stamps.length === 0) {
      if (text && !/^\[[a-z]+:.*\]$/i.test(raw.trim())) lines.push({ timeMs: -1, text });
      continue;
    }
    timed = true;
    for (const m of stamps) {
      const frac = m[3] ? Number(m[3].padEnd(3, "0")) : 0;
      lines.push({ timeMs: Number(m[1]) * 60_000 + Number(m[2]) * 1000 + frac, text });
    }
  }
  if (!timed) return { synced: false, lines };
  return { synced: true, lines: lines.filter((l) => l.timeMs >= 0).sort((a, b) => a.timeMs - b.timeMs) };
}

/** index of the line being sung at `positionMs`, or -1 before the first */
export function activeLineIndex(lines: LyricLine[], positionMs: number): number {
  let lo = 0;
  let hi = lines.length - 1;
  let found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].timeMs <= positionMs) {
      found = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return found;
}
