import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { useSettings } from "@/settings/store";
import type { Track } from "@/types";
import { syncSoundCloudOnStart } from "@/services/soundcloud/account";
import { startMirror } from "@/services/soundcloud/mirror";

// Picking up where you left off: the queue and the position are saved now and then and
// restored on start (paused). Also: pause when the headphones go away.

const KEY = "evplayer.session.v1";

interface Saved {
  queue: Track[];
  index: number;
  positionMs: number;
}

let lastSaved = "";

function save(): void {
  const s = usePlayer.getState();
  if (!s.queue.length) return;
  // nothing new (paused, or the same song a few seconds on): no rewrite of the whole queue
  const key = `${s.queue.length}|${s.queue[0]?.id}|${s.queue[s.queue.length - 1]?.id}|${s.index}|${Math.round(s.positionMs / 5000)}`;
  if (key === lastSaved) return;
  lastSaved = key;
  // a long queue is cut around the current song so the file stays small
  const start = Math.max(0, s.index - 50);
  const data: Saved = { queue: s.queue.slice(start, start + 300), index: s.index - start, positionMs: s.positionMs };
  try {
    localStorage.setItem(KEY, JSON.stringify(data));
  } catch {
    // not important
  }
}

export function restoreSession(): void {
  if (useSettings.getState().resumeOnStart) {
    try {
      const data = JSON.parse(localStorage.getItem(KEY) ?? "null") as Saved | null;
      if (data?.queue?.length && data.queue[data.index]) player.restore(data.queue, data.index, data.positionMs);
    } catch {
      // a broken save is ignored
    }
  }
  setInterval(save, 10_000);
  window.addEventListener("beforeunload", save);
  watchOutputDevices();
  syncSoundCloudOnStart();
  startMirror();
}

/**
 * Whether a sound output went away between two looks at the list. The browser may hide the
 * ids until a microphone is allowed (all are then empty), so with no ids the count decides.
 */
export function outputLost(before: string[], after: string[]): boolean {
  if (before.some(Boolean)) return before.filter(Boolean).some((id) => !after.includes(id));
  return after.length < before.length;
}

function watchOutputDevices(): void {
  const devices = navigator.mediaDevices;
  if (!devices?.enumerateDevices) return;
  let outputs: string[] | null = null;
  const read = async () => (await devices.enumerateDevices()).filter((d) => d.kind === "audiooutput").map((d) => d.deviceId).sort();
  void read().then((v) => (outputs = v));
  devices.addEventListener("devicechange", async () => {
    const now = await read();
    const lost = outputs !== null && outputLost(outputs, now);
    outputs = now;
    if (lost && useSettings.getState().pauseOnDeviceChange && usePlayer.getState().status === "playing") player.toggle();
  });
}
