import type { SettingsState } from "@/settings/store";
import type { Sample } from "./probe";

// What to do about what the diagnostics page measured: each piece of advice says what costs
// what and, where a setting is the cure, carries the change that applies it.

type Knobs = Pick<SettingsState, "animatedBackdrop" | "memorySaver" | "overlayEnabled" | "coverGlow" | "lyricsBlur">;

export interface Advice {
  id: string;
  level: "high" | "medium" | "info";
  title: string;
  detail: string;
  /** the settings that fix it */
  fix?: { label: string; patch: Partial<Knobs> };
}

const MB = 1024 * 1024;
export const mb = (bytes: number) => (bytes < MB ? `${Math.max(1, Math.round(bytes / 1024))} КБ` : `${Math.round(bytes / MB)} МБ`);
const pct = (share: number) => `${Math.round(share * 100)}%`;

export function advise(s: Sample, o: Knobs): Advice[] {
  const out: Advice[] = [];
  const total = s.processes.reduce((n, p) => n + p.memory, 0);
  const cpu = s.processes.reduce((n, p) => n + p.cpu, 0);

  // the background: drawn many times a second on the main thread and the graphics card
  const backdropShare = (s.backdrop.fps * s.backdrop.msPerFrame) / 1000;
  if (o.animatedBackdrop && s.backdrop.fps > 1) {
    out.push({
      id: "backdrop",
      level: backdropShare > 0.04 || cpu > 8 ? "medium" : "info",
      title: "Живой фон",
      detail: `Рисуется ${Math.round(s.backdrop.fps)} раз в секунду и занимает ${pct(backdropShare)} главного потока, а основную работу делает видеокарта. Выключенный фон стоит на месте и ничего не тратит.`,
      fix: { label: "Выключить живой фон", patch: { animatedBackdrop: false } },
    });
  }

  const heap = s.heap?.used ?? 0;
  if (!o.memorySaver && (heap > 180 * MB || total > 700 * MB)) {
    out.push({
      id: "memory",
      level: heap > 300 * MB || total > 1000 * MB ? "high" : "medium",
      title: "Много памяти",
      detail: `Приложение занимает ${mb(total)}${heap ? `, из них данные страницы — ${mb(heap)}` : ""}. Экономия памяти убирает тяжёлые эффекты, держит меньше картинок и отдаёт память системе, когда окно свёрнуто.`,
      fix: { label: "Включить экономию памяти", patch: { memorySaver: true } },
    });
  }

  if (s.busy > 0.1 || s.lag > 50) {
    out.push({
      id: "busy",
      level: s.busy > 0.25 || s.lag > 120 ? "high" : "medium",
      title: "Интерфейс подтормаживает",
      detail: `Главный поток занят долгими задачами на ${pct(s.busy)}, таймеры опаздывают в среднем на ${Math.round(s.lag)} мс.${s.domNodes > 4000 ? ` На открытой странице ${s.domNodes} элементов: длинный список тяжелее всего.` : ""}`,
      ...(o.animatedBackdrop ? { fix: { label: "Выключить живой фон", patch: { animatedBackdrop: false } } } : {}),
    });
  } else if (s.domNodes > 6000) {
    out.push({
      id: "dom",
      level: "info",
      title: "Большая страница",
      detail: `На открытой странице ${s.domNodes} элементов. Когда она не нужна, перейдите на другую: память освободится.`,
    });
  }

  if ((o.coverGlow || o.lyricsBlur) && cpu > 8) {
    out.push({
      id: "effects",
      level: "medium",
      title: "Эффекты нагружают видеокарту",
      detail: `Процессор занят на ${Math.round(cpu)}%. Свечение обложки и размытие строк текста песни перерисовываются при каждом движении.`,
      fix: { label: "Выключить свечение и размытие", patch: { coverGlow: false, lyricsBlur: false } },
    });
  }

  if (o.overlayEnabled) {
    out.push({
      id: "overlay",
      level: "info",
      title: "Мини-плеер",
      detail: "Это ещё одно окно со своей страницей: обычно 50–100 МБ памяти. Если им не пользуетесь, его можно выключить.",
      fix: { label: "Выключить мини-плеер", patch: { overlayEnabled: false } },
    });
  }

  const stored = s.storage.reduce((n, x) => n + x.bytes, 0);
  if (stored > 3 * MB) {
    out.push({
      id: "storage",
      level: "info",
      title: "Сохранённые данные",
      detail: `Страница хранит ${mb(stored)}; больше всего — ${s.storage.slice(0, 3).map((x) => `${x.key} (${mb(x.bytes)})`).join(", ")}. Лишнее можно не хранить: Настройки → Данные и копия.`,
    });
  }

  if (s.soundGraph && cpu > 15) {
    out.push({
      id: "sound",
      level: "info",
      title: "Обработка звука",
      detail: "Эквалайзер, выравнивание громкости, реверберация, смена тона и кроссфейд считаются на процессоре. Если что-то из этого не нужно, выключите его в Настройках → Звук.",
    });
  }

  const rank = { high: 0, medium: 1, info: 2 } as const;
  return out.sort((a, b) => rank[a.level] - rank[b.level]);
}

/** everything measured, as text to send to the developer */
export function report(s: Sample, advice: Advice[]): string {
  const lines = [
    `EvPlayer — диагностика, ${new Date(s.at).toLocaleString()}`,
    `Процессы: ${s.processes.map((p) => `${p.name} #${p.pid} ${mb(p.memory)} ЦП ${p.cpu.toFixed(1)}%`).join("; ") || "нет данных"}`,
    `Память страницы: ${s.heap ? `${mb(s.heap.used)} из ${mb(s.heap.total)}` : "нет данных"}; элементов на странице: ${s.domNodes}`,
    `Главный поток: занят ${pct(s.busy)}, опоздание таймеров ${Math.round(s.lag)} мс`,
    `Фон: ${Math.round(s.backdrop.fps)} кадров/с, ${s.backdrop.msPerFrame.toFixed(2)} мс на кадр; обработка звука: ${s.soundGraph ? "да" : "нет"}`,
    `Данные: лайков ${s.counts.likes}, плейлистов ${s.counts.playlists} (${s.counts.playlistTracks} треков), история ${s.counts.history}, очередь ${s.counts.queue}, офлайн ${s.counts.offline}, в сессии ${s.counts.jamPeople}`,
    `Хранилище: ${s.storage.slice(0, 8).map((x) => `${x.key} ${mb(x.bytes)}`).join(", ")}`,
    `Советы: ${advice.map((a) => a.title).join(", ") || "нет"}`,
  ];
  return lines.join("\n");
}
