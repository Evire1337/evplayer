import type { ProcessInfo } from "@/platform/api";
import { backdropStats } from "@/ui/backdrop/BackdropRenderer";
import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { useLibrary } from "@/library/store";
import { useHistory } from "@/stats/history";
import { useOffline } from "@/offline/store";
import { useJam } from "@/jam/store";

// The measuring behind the diagnostics page. It runs only while the page is open: a long-task
// observer (how busy the page's main thread is), a timer that notes how late it fires (how
// sluggish the page feels), and every few seconds a reading of the app's processes, the page's
// memory and what the background costs to draw.

export interface ProcessLoad extends ProcessInfo {
  /** share of the whole processor (all cores), 0..100, as Task Manager shows it */
  cpu: number;
}

export interface Sample {
  at: number;
  processes: ProcessLoad[];
  /** the page's JavaScript memory (Chromium only) */
  heap?: { used: number; total: number };
  domNodes: number;
  /** share of the main thread taken by long tasks, 0..1 */
  busy: number;
  /** how late a timer fires on average, ms */
  lag: number;
  backdrop: { fps: number; msPerFrame: number };
  soundGraph: boolean;
  storage: Array<{ key: string; bytes: number }>;
  counts: { likes: number; playlists: number; playlistTracks: number; history: number; queue: number; offline: number; jamPeople: number };
}

interface Raw {
  at: number;
  cpu: Map<number, number>;
  frames: number;
  frameMs: number;
}

const LAG_EVERY_MS = 250;

export class Probe {
  private observer: PerformanceObserver | null = null;
  private longMs = 0;
  private lagTotal = 0;
  private lagCount = 0;
  private lagTimer: ReturnType<typeof setTimeout> | undefined;
  private last: Raw | null = null;

  start(): void {
    try {
      this.observer = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) this.longMs += entry.duration;
      });
      this.observer.observe({ type: "longtask", buffered: false });
    } catch {
      this.observer = null; // not in this engine: the share stays 0
    }
    let expected = performance.now() + LAG_EVERY_MS;
    const tick = () => {
      const now = performance.now();
      this.lagTotal += Math.max(0, now - expected);
      this.lagCount++;
      expected = now + LAG_EVERY_MS;
      this.lagTimer = setTimeout(tick, LAG_EVERY_MS);
    };
    this.lagTimer = setTimeout(tick, LAG_EVERY_MS);
  }

  stop(): void {
    this.observer?.disconnect();
    this.observer = null;
    clearTimeout(this.lagTimer);
  }

  /** a reading; the shares are over the time since the last one */
  async sample(): Promise<Sample> {
    const at = performance.now();
    const processes = (await window.evplayer?.processes().catch(() => [])) ?? [];
    const prev = this.last;
    const wall = prev ? at - prev.at : 0;
    const cores = navigator.hardwareConcurrency || 1;
    const loads = processes.map((p) => {
      const before = prev?.cpu.get(p.pid);
      const cpu = wall > 0 && before !== undefined ? Math.max(0, ((p.cpuMs - before) / wall / cores) * 100) : 0;
      return { ...p, cpu };
    });
    const frames = backdropStats.frames - (prev?.frames ?? backdropStats.frames);
    const frameMs = backdropStats.busyMs - (prev?.frameMs ?? backdropStats.busyMs);
    this.last = { at, cpu: new Map(processes.map((p) => [p.pid, p.cpuMs])), frames: backdropStats.frames, frameMs: backdropStats.busyMs };

    const memory = (performance as Performance & { memory?: { usedJSHeapSize: number; totalJSHeapSize: number } }).memory;
    const busy = wall > 0 ? Math.min(1, this.longMs / wall) : 0;
    const lag = this.lagCount ? this.lagTotal / this.lagCount : 0;
    this.longMs = 0;
    this.lagTotal = 0;
    this.lagCount = 0;

    const lib = useLibrary.getState();
    return {
      at: Date.now(),
      processes: loads.sort((a, b) => b.memory - a.memory),
      heap: memory ? { used: memory.usedJSHeapSize, total: memory.totalJSHeapSize } : undefined,
      domNodes: document.getElementsByTagName("*").length,
      busy,
      lag,
      backdrop: { fps: wall > 0 ? (frames * 1000) / wall : 0, msPerFrame: frames ? frameMs / frames : 0 },
      soundGraph: player.soundGraphOn,
      storage: storageSizes(),
      counts: {
        likes: lib.likes.length,
        playlists: lib.playlists.length,
        playlistTracks: lib.playlists.reduce((n, p) => n + p.items.length, 0),
        history: useHistory.getState().entries.length,
        queue: usePlayer.getState().queue.length,
        offline: Object.keys(useOffline.getState().entries).length,
        jamPeople: useJam.getState().status === "live" ? useJam.getState().people.length : 0,
      },
    };
  }
}

/** what each saved key takes (the page's storage keeps text as two bytes a character) */
export function storageSizes(): Array<{ key: string; bytes: number }> {
  const out: Array<{ key: string; bytes: number }> = [];
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key) out.push({ key, bytes: (localStorage.getItem(key)?.length ?? 0) * 2 });
    }
  } catch {
    // storage closed off: nothing to show
  }
  return out.sort((a, b) => b.bytes - a.bytes);
}
