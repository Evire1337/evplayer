import { mediaStream } from "@/platform/net";
import { player } from "@/player/engine";
import { streamCandidates } from "@/player/streams";
import { currentTrack, usePlayer } from "@/player/store";
import { useSettings } from "@/settings/store";
import type { Track } from "@/types";
import { fetchWhole, isWhole } from "./download";
import { MAX_STEM_SECONDS, stemGains, stemKey } from "./model";
import { setJob, useStems } from "./store";

// The switch of "Дорожки": while it is on the networks are in memory and the songs that are
// playing and coming up are split in the background, one at a time; when it is off nothing is
// loaded and nothing runs (the split songs stay on disk, ready for next time).

/** how many songs after the current one are split ahead of time */
const AHEAD = 3;

const keys = new Map<string, string>();
let running = false;
let controller: AbortController | null = null;
let offProgress: (() => void) | null = null;
let offModel: (() => void) | null = null;
let offPlayer: (() => void) | null = null;
let offStems: (() => void) | null = null;
let offSettings: (() => void) | null = null;
const queue: Track[] = [];
/** the song being split now */
let runningId: string | null = null;

const api = () => window.evplayer?.stems;

/** the key a song's tracks are stored under, once it has been looked at */
export const keyOf = (trackId: string): string | undefined => keys.get(trackId);

/** the song's sound as 16-bit stereo at 44.1 kHz, the way the splitter takes it */
async function decode(track: Track, signal: AbortSignal): Promise<Uint8Array> {
  let url: string | null = null;
  // the first way to the sound that is a plain file (a stream cut into pieces cannot be fetched whole)
  for await (const stream of streamCandidates(track, signal)) {
    if (!stream.hls) {
      url = mediaStream(stream.url);
      break;
    }
  }
  if (!url) throw new Error("у этой песни нет целого файла, который можно разделить");
  const data = await fetchWhole(url, signal);
  // the context's rate is the one the sound comes out at
  const audio = await new OfflineAudioContext(2, 1, 44100).decodeAudioData(data);
  // a part of the song would be split as if it were the song, and its tracks would end early
  if (!isWhole(audio.duration, track.durationMs / 1000)) throw new Error("звук скачался не целиком, песню не разделить");
  const left = audio.getChannelData(0);
  const right = audio.numberOfChannels > 1 ? audio.getChannelData(1) : left;
  const pcm = new Int16Array(audio.length * 2);
  for (let i = 0; i < audio.length; i++) {
    pcm[i * 2] = Math.max(-32768, Math.min(32767, Math.round(left[i] * 32767)));
    pcm[i * 2 + 1] = Math.max(-32768, Math.min(32767, Math.round(right[i] * 32767)));
  }
  return new Uint8Array(pcm.buffer);
}

/** the sound of the next song, fetched while the current one is split (the network and the processor are busy with different things) */
const prefetched = new Map<string, Promise<Uint8Array>>();

async function prefetch(track: Track, key: string): Promise<void> {
  const stems = api();
  if (!stems || !controller || prefetched.has(track.id)) return;
  const signal = controller.signal;
  // a song that is already split needs no sound
  if (await stems.meta(key).catch(() => null)) return;
  if (signal.aborted || prefetched.has(track.id)) return;
  const sound = decode(track, signal);
  sound.catch(() => prefetched.delete(track.id));
  prefetched.set(track.id, sound);
}

async function work(track: Track, key: string): Promise<void> {
  const stems = api();
  if (!stems || !controller) return;
  const signal = controller.signal;
  const cached = await stems.meta(key);
  if (cached) {
    setJob(key, { status: "ready", share: 1, meta: cached });
    return;
  }
  setJob(key, { status: "decoding", share: 0.02 });
  // the sound may have been fetched already, while the song before was split
  let pcm = await prefetched.get(track.id)?.catch(() => undefined);
  prefetched.delete(track.id);
  pcm ??= await decode(track, signal);
  if (signal.aborted) return;
  setJob(key, { status: "separating", share: 0.1 });
  // the next one's sound comes in the meantime
  const next = queue[0];
  const nextKey = next && keys.get(next.id);
  if (next && nextKey) void prefetch(next, nextKey);
  const meta = await stems.separate(key, pcm, useSettings.getState().stemsCacheMb * 1_000_000);
  setJob(key, { status: "ready", share: 1, meta });
  void refreshCache();
}

/** one song after another; the page and the player are never kept waiting */
async function drain(): Promise<void> {
  if (running) return;
  running = true;
  try {
    while (queue.length && useStems.getState().phase === "ready") {
      const track = queue.shift()!;
      const key = keys.get(track.id);
      if (!key || useStems.getState().jobs[key]?.status === "ready") continue;
      runningId = track.id;
      try {
        await work(track, key);
      } catch (error) {
        if (!controller || controller.signal.aborted) break;
        const message = error instanceof Error ? error.message : String(error);
        if (/отменено/.test(message)) {
          // skipped past: not a failure, and it can be asked for again
          keys.delete(track.id);
          useStems.setState((st) => ({ jobs: Object.fromEntries(Object.entries(st.jobs).filter(([k]) => k !== key)) }));
        } else {
          setJob(key, { status: "error", error: message });
        }
      }
    }
  } finally {
    running = false;
    runningId = null;
  }
}

async function refreshCache(): Promise<void> {
  const state = await api()?.state().catch(() => null);
  if (state) useStems.setState({ cacheBytes: state.cacheBytes });
}

/** the songs to split now: the current one, then the next few */
function wanted(): Track[] {
  const s = usePlayer.getState();
  return s.queue.slice(Math.max(0, s.index), s.index + 1 + AHEAD);
}

function enqueue(track: Track): void {
  if (track.durationMs / 1000 > MAX_STEM_SECONDS) return;
  if (queue.some((q) => q.id === track.id)) return;
  void stemKey(track.id, track.durationMs).then((key) => {
    keys.set(track.id, key);
    const job = useStems.getState().jobs[key];
    if (job && job.status !== "error") {
      reconcile();
      return;
    }
    setJob(key, { status: "queued", share: 0, error: undefined });
    queue.push(track);
    void drain();
  });
}

/** what the player is doing with the tracks: split what is coming, play the split tracks when they are ready */
export function reconcile(): void {
  const st = useStems.getState();
  if (st.phase !== "ready") {
    if (st.engaged) player.releaseStems();
    return;
  }
  const coming = wanted();
  // songs that are no longer current or next (skipped past) are let go, and the one playing goes first
  const ids = new Set(coming.map((t) => t.id));
  for (let i = queue.length - 1; i >= 0; i--) {
    if (ids.has(queue[i].id)) continue;
    const key = keys.get(queue[i].id);
    queue.splice(i, 1);
    if (key) useStems.setState((s) => ({ jobs: Object.fromEntries(Object.entries(s.jobs).filter(([k]) => k !== key)) }));
  }
  if (runningId && !ids.has(runningId)) void api()?.cancel();
  const now = coming[0]?.id;
  queue.sort((a, b) => Number(b.id === now) - Number(a.id === now));
  for (const t of coming) {
    if (!keys.has(t.id)) enqueue(t);
  }
  const cur = currentTrack();
  const status = usePlayer.getState().status;
  const key = cur ? keys.get(cur.id) : undefined;
  // the tracks start on the first seconds, while the rest of the song is still being split
  const job = key ? st.jobs[key] : undefined;
  const ready = !!key && !!job && !st.broken[key] && (job.status === "ready" || (job.status === "separating" && job.share > 0.1));
  if (key && ready && (status === "playing" || status === "paused") && st.engaged !== key) {
    player.engageStems(key, () => useStems.getState().jobs[key]?.status === "ready");
  }
  else if (st.engaged && st.engaged !== key) player.releaseStems();
}

function applyGains(): void {
  const s = useSettings.getState();
  player.setStemGains(stemGains(s.stemFaders, s.stemMuted, s.stemSolo));
}

/** switch on: the model (once), the networks, the background splitting */
export async function startStems(): Promise<void> {
  const stems = api();
  if (!stems) {
    useStems.setState({ phase: "error", error: "Разделение на дорожки работает только в приложении для компьютера" });
    return;
  }
  if (useStems.getState().phase === "ready" || useStems.getState().phase === "loading" || useStems.getState().phase === "download") return;
  controller = new AbortController();
  try {
    const state = await stems.state();
    if (!state.supported) throw new Error("Разделение на дорожки есть только на компьютере");
    useStems.setState({ cacheBytes: state.cacheBytes, error: undefined });
    if (!state.modelReady) {
      useStems.setState({ phase: "download", modelShare: 0 });
      offModel = stems.onModel((done, total) => useStems.setState({ modelShare: total > 0 ? Math.min(1, done / total) : 0 }));
      await stems.downloadModel();
      offModel?.();
      offModel = null;
    }
    // switched off while the model downloaded
    if (!useSettings.getState().stemsOn) {
      useStems.setState({ phase: "off" });
      return;
    }
    useStems.setState({ phase: "loading" });
    await stems.load();
    if (!useSettings.getState().stemsOn) {
      await stems.unload();
      useStems.setState({ phase: "off" });
      return;
    }
    offProgress = stems.onProgress((key, share) => setJob(key, { status: "separating", share: 0.1 + share * 0.9 }));
    useStems.setState({ phase: "ready" });
    offPlayer = usePlayer.subscribe((s, prev) => {
      if (s.index !== prev.index || s.queue !== prev.queue || s.status !== prev.status) reconcile();
    });
    offStems = useStems.subscribe((s, prev) => {
      if (s.jobs !== prev.jobs || s.tick !== prev.tick) reconcile();
    });
    offSettings = useSettings.subscribe((s, prev) => {
      if (s.stemFaders !== prev.stemFaders || s.stemMuted !== prev.stemMuted || s.stemSolo !== prev.stemSolo) applyGains();
    });
    reconcile();
  } catch (error) {
    offModel?.();
    offModel = null;
    useStems.setState({ phase: "error", error: error instanceof Error ? error.message : String(error) });
  }
}

/** switch off: everything stops and is let go; the split songs stay on disk */
export async function stopStems(): Promise<void> {
  controller?.abort();
  controller = null;
  queue.length = 0;
  offProgress?.();
  offModel?.();
  offPlayer?.();
  offStems?.();
  offSettings?.();
  offProgress = offModel = offPlayer = offStems = offSettings = null;
  keys.clear();
  prefetched.clear();
  player.releaseStems(true);
  useStems.setState({ phase: "off", modelShare: 0, error: undefined, jobs: {}, engaged: null, broken: {} });
  const stems = api();
  if (stems) {
    await stems.cancel().catch(() => undefined);
    await stems.unload().catch(() => undefined);
    void refreshCache();
  }
}

export async function clearStemsCache(): Promise<void> {
  // the tracks that play are let go first: their files are about to be deleted
  player.releaseStems();
  useStems.setState((s) => ({ jobs: Object.fromEntries(Object.entries(s.jobs).filter(([, j]) => j.status !== "ready")), broken: {} }));
  await api()?.clearCache();
  await refreshCache();
  // what is playing is split again
  keys.clear();
  reconcile();
}

export { applyGains };

/** the setting decides: called when the app starts and whenever the switch moves */
export function followSwitch(): void {
  let was = false;
  const apply = () => {
    const on = useSettings.getState().stemsOn;
    if (on === was) return;
    was = on;
    void (on ? startStems() : stopStems());
  };
  useSettings.subscribe(apply);
  apply();
}
