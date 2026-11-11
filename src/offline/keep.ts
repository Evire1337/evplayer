import type { Track } from "@/types";
import type { OfflineSource } from "@/platform/api";
import { soundCloudStreams } from "@/services/soundcloud/api";
import { resolveYouTubeAudio, videoIdOf } from "@/services/youtube/direct";
import { toast } from "@/ui/toasts";
import { useKeepLists, useOffline } from "./store";
import { isKeepable } from "./scopes";

// Keeping songs: a queue that fetches a few at a time. Each song is asked for exactly as
// the player would ask for it to play (SoundCloud's own formats, YouTube's audio), so only
// what can be played can be kept; the desktop side does the downloading itself.

const PARALLEL = 2;
/** a song that has said nothing for this long is given up on (the connection hangs, the service does not answer) */
export const STALL_MS = 60_000;
const WATCH_EVERY_MS = 5_000;

export interface KeepJob {
  total: number;
  done: number;
  failed: number;
  /** the first reason a song could not be kept, for the summary */
  reason: string | null;
}

interface Queue {
  waiting: Track[];
  running: Map<string, AbortController>;
  job: KeepJob | null;
}

const queue: Queue = { waiting: [], running: new Map(), job: null };
/** when each running song last made progress (or started) */
const lastSeen = new Map<string, number>();
/** songs given up on because they stalled: counted as failed, not as cancelled */
const stalled = new Set<string>();
let watchdog: ReturnType<typeof setInterval> | undefined;

/** the desktop side said a song is coming in: it is alive */
export function noteProgress(id: string): void {
  if (queue.running.has(id)) lastSeen.set(id, Date.now());
}

function watch(): void {
  if (watchdog) return;
  watchdog = setInterval(() => {
    for (const [id, controller] of queue.running) {
      if (Date.now() - (lastSeen.get(id) ?? Date.now()) < STALL_MS) continue;
      stalled.add(id);
      controller.abort();
      window.evplayer?.offline.cancel(id);
    }
    if (!queue.running.size) {
      clearInterval(watchdog);
      watchdog = undefined;
    }
  }, WATCH_EVERY_MS);
}

/** waits for a promise, but lets go of it as soon as the signal says so (a hung download would never return) */
function until<T>(promise: Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const abort = () => reject(new DOMException("aborted", "AbortError"));
    if (signal.aborted) return abort();
    signal.addEventListener("abort", abort, { once: true });
    promise.then(resolve, reject).finally(() => signal.removeEventListener("abort", abort));
  });
}
const listeners = new Set<(job: KeepJob | null) => void>();

const emit = () => listeners.forEach((l) => l(queue.job ? { ...queue.job } : null));

/** the current batch, for the progress panel */
export function watchJob(listener: (job: KeepJob | null) => void): () => void {
  listeners.add(listener);
  listener(queue.job ? { ...queue.job } : null);
  return () => void listeners.delete(listener);
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** a stream address to fetch for a track, best first */
async function* sources(track: Track, signal: AbortSignal): AsyncGenerator<OfflineSource> {
  if (track.source === "soundcloud") {
    for await (const stream of soundCloudStreams(track.id, signal)) {
      yield stream.playlist ? { kind: "hls", url: stream.playlist } : { kind: stream.hls ? "hls" : "file", url: stream.url };
    }
    return;
  }
  const videoId = videoIdOf(track.id);
  if (!videoId) throw new Error("не похоже на видео YouTube");
  const audio = await resolveYouTubeAudio(videoId, signal);
  // YouTube gives the song in pieces and needs to know its length; it is in the address
  const url = new URL(audio.url);
  if (!url.searchParams.has("clen") && audio.contentLength) url.searchParams.set("clen", String(audio.contentLength));
  yield { kind: "file", url: url.toString() };
}

async function keepOne(track: Track, signal: AbortSignal): Promise<void> {
  const api = window.evplayer!.offline;
  let last: unknown = new Error("нет подходящего формата");
  for await (const source of sources(track, signal)) {
    try {
      lastSeen.set(track.id, Date.now());
      const entry = await until(api.download(track.id, source), signal);
      useOffline.setState((s) => ({ entries: { ...s.entries, [track.id]: entry } }));
      markFailed(track.id, false);
      return;
    } catch (error) {
      if (signal.aborted) throw error;
      last = error;
    }
  }
  throw last;
}

function finish(): void {
  const job = queue.job;
  queue.job = null;
  emit();
  if (!job) return;
  const kept = job.done - job.failed;
  if (job.done < job.total) return; // cancelled
  if (job.failed === 0) toast(kept === 1 ? "Трек сохранён для офлайн" : `Для офлайн сохранено: ${kept}`);
  else toast(`Сохранено ${kept} из ${job.total}. Не получилось: ${job.failed}${job.reason ? ` — ${job.reason}` : ""}`, kept ? "ok" : "error");
}

async function pump(): Promise<void> {
  while (queue.running.size < PARALLEL && queue.waiting.length) {
    const track = queue.waiting.shift()!;
    const controller = new AbortController();
    queue.running.set(track.id, controller);
    lastSeen.set(track.id, Date.now());
    watch();
    void (async () => {
      try {
        await keepOne(track, controller.signal);
      } catch (error) {
        if (!controller.signal.aborted || stalled.has(track.id)) {
          markFailed(track.id, true);
          if (queue.job) {
            queue.job.failed++;
            queue.job.reason ??= stalled.has(track.id) ? "сервис не отвечает" : messageOf(error);
          }
        }
      } finally {
        const given = !controller.signal.aborted || stalled.has(track.id);
        queue.running.delete(track.id);
        lastSeen.delete(track.id);
        stalled.delete(track.id);
        useOffline.setState((s) => {
          const { [track.id]: _gone, ...rest } = s.progress;
          return { progress: rest };
        });
        if (queue.job && given) queue.job.done++;
        emit();
        if (queue.waiting.length) void pump();
        else if (queue.running.size === 0) finish();
      }
    })();
  }
}

function markFailed(id: string, failed: boolean): void {
  useKeepLists.setState((s) => {
    const has = s.failed.includes(id);
    if (has === failed) return s;
    return { failed: failed ? [...s.failed, id] : s.failed.filter((f) => f !== id) };
  });
}

/**
 * Add songs to the queue; the ones already kept or already waiting are skipped. Returns how
 * many were new. uto: a list topping itself up, which leaves out songs that failed before
 * (asked for by hand, they are tried again).
 */
export function keepTracks(tracks: Track[], { auto = false } = {}): number {
  if (!window.evplayer?.offline) {
    toast("Сохранение для офлайн работает только в приложении", "error");
    return 0;
  }
  const have = useOffline.getState().entries;
  const queued = new Set([...queue.waiting.map((t) => t.id), ...queue.running.keys()]);
  const failed = new Set(auto ? useKeepLists.getState().failed : []);
  const fresh = tracks.filter((t) => isKeepable(t) && !(t.id in have) && !failed.has(t.id) && !queued.has(t.id) && queued.add(t.id));
  if (!fresh.length) return 0;
  queue.job ??= { total: 0, done: 0, failed: 0, reason: null };
  queue.job.total += fresh.length;
  queue.waiting.push(...fresh);
  emit();
  void pump();
  return fresh.length;
}

/** stop everything that is waiting or running; what is already kept stays */
export function cancelKeeping(): void {
  queue.waiting = [];
  for (const [id, controller] of queue.running) {
    controller.abort();
    window.evplayer?.offline.cancel(id);
  }
  queue.running.clear();
  queue.job = null;
  emit();
}

/** delete the kept copies of these songs */
export async function forgetTracks(ids: string[]): Promise<void> {
  const api = window.evplayer?.offline;
  if (!api || !ids.length) return;
  await api.remove(ids);
  useOffline.setState((s) => ({ entries: Object.fromEntries(Object.entries(s.entries).filter(([id]) => !ids.includes(id))) }));
}

/** progress events of the desktop side → the page (once, at start) */
export function listenForProgress(): () => void {
  return (
    window.evplayer?.offline.onProgress(({ id, done, total }) => {
      noteProgress(id);
      useOffline.setState((s) => ({ progress: { ...s.progress, [id]: total > 0 ? Math.min(1, done / total) : -1 } }));
    }) ?? (() => undefined)
  );
}
