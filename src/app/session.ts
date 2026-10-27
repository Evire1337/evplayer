import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { useSettings } from "@/settings/store";
import type { Track } from "@/types";
import { syncSoundCloudOnStart } from "@/services/soundcloud/account";

// Picking up where you left off: the queue and the position are saved now and then and
// restored on start (paused). Also: pause when the headphones go away.

const KEY = "evplayer.session.v1";

interface Saved {
  queue: Track[];
  index: number;
  positionMs: number;
}

function save(): void {
  const s = usePlayer.getState();
  if (!s.queue.length) return;
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
}

function watchOutputDevices(): void {
  const devices = navigator.mediaDevices;
  if (!devices?.enumerateDevices) return;
  let outputs = "";
  const read = async () => (await devices.enumerateDevices()).filter((d) => d.kind === "audiooutput").map((d) => d.deviceId).sort().join(",");
  void read().then((v) => (outputs = v));
  devices.addEventListener("devicechange", async () => {
    const now = await read();
    const lost = outputs && now.split(",").length < outputs.split(",").length;
    outputs = now;
    if (lost && useSettings.getState().pauseOnDeviceChange && usePlayer.getState().status === "playing") player.toggle();
  });
}
