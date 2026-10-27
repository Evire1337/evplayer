const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** "5 мин. назад", "вчера", "2 нед. назад", then a date: like the "Date added" column */
export function formatAddedAt(iso: string | undefined, now = Date.now()): string {
  if (!iso) return "";
  const time = new Date(iso).getTime();
  if (!Number.isFinite(time)) return "";
  const diff = Math.max(0, now - time);
  const rtf = new Intl.RelativeTimeFormat("ru", { numeric: "auto", style: "short" });
  if (diff < MINUTE) return rtf.format(0, "second");
  if (diff < HOUR) return rtf.format(-Math.floor(diff / MINUTE), "minute");
  if (diff < DAY) return rtf.format(-Math.floor(diff / HOUR), "hour");
  if (diff < 7 * DAY) return rtf.format(-Math.floor(diff / DAY), "day");
  if (diff < 30 * DAY) return rtf.format(-Math.floor(diff / (7 * DAY)), "week");
  return formatDate(iso);
}

export function formatDate(iso: string): string {
  return new Intl.DateTimeFormat("ru", { day: "numeric", month: "short", year: "numeric" }).format(new Date(iso));
}
