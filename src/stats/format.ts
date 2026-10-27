/** listening time in words: "3 ч 12 мин", "45 мин", "40 с" */
export function formatListenTime(ms: number): string {
  const totalMin = Math.floor(ms / 60000);
  if (totalMin < 1) return `${Math.max(0, Math.round(ms / 1000))} с`;
  const h = Math.floor(totalMin / 60);
  const m = totalMin % 60;
  return h ? `${h} ч ${m} мин` : `${m} мин`;
}

/** "1 прослушивание", "3 прослушивания", "5 прослушиваний" */
export function plural(n: number, one: string, few: string, many: string): string {
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return one;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return few;
  return many;
}

export const playsWord = (n: number) => `${n} ${plural(n, "прослушивание", "прослушивания", "прослушиваний")}`;
