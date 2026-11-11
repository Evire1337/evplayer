import { describe, expect, it } from "vitest";
import { advise, report } from "./advice";
import type { Sample } from "./probe";

const MB = 1024 * 1024;

const sample = (patch: Partial<Sample> = {}): Sample => ({
  at: 0,
  processes: [{ pid: 1, name: "evplayer.exe", memory: 80 * MB, cpuMs: 0, cpu: 1 }],
  heap: { used: 60 * MB, total: 90 * MB },
  domNodes: 1500,
  busy: 0,
  lag: 2,
  backdrop: { fps: 0, msPerFrame: 0 },
  soundGraph: false,
  storage: [],
  counts: { likes: 0, playlists: 0, playlistTracks: 0, history: 0, queue: 0, offline: 0, jamPeople: 0 },
  ...patch,
});

const calm = { animatedBackdrop: false, memorySaver: false, overlayEnabled: false, coverGlow: false, lyricsBlur: false };

describe("diagnostics advice", () => {
  it("has nothing to say when little is used", () => {
    expect(advise(sample(), calm)).toEqual([]);
  });

  it("names the background with what it costs and how to turn it off", () => {
    const [a] = advise(sample({ backdrop: { fps: 30, msPerFrame: 2 } }), { ...calm, animatedBackdrop: true });
    expect(a.id).toBe("backdrop");
    expect(a.detail).toContain("30 раз в секунду");
    expect(a.detail).toContain("6%");
    expect(a.fix?.patch).toEqual({ animatedBackdrop: false });
  });

  it("puts what matters most first", () => {
    const list = advise(sample({ heap: { used: 400 * MB, total: 500 * MB }, busy: 0.3 }), { ...calm, overlayEnabled: true });
    expect(list.map((a) => a.level)).toEqual(["high", "high", "info"]);
    expect(list.find((a) => a.id === "memory")?.fix?.patch).toEqual({ memorySaver: true });
    // with the saver already on, memory is not brought up again
    expect(advise(sample({ heap: { used: 400 * MB, total: 500 * MB } }), { ...calm, memorySaver: true }).some((a) => a.id === "memory")).toBe(false);
  });

  it("counts the processes' own memory: a few hundred MB is normal, more is worth a word", () => {
    const procs = (mbEach: number) => [1, 2, 3].map((pid) => ({ pid, name: "msedgewebview2.exe", memory: mbEach * MB, cpuMs: 0, cpu: 0 }));
    expect(advise(sample({ processes: procs(100) }), calm).some((a) => a.id === "memory")).toBe(false);
    expect(advise(sample({ processes: procs(170) }), calm).find((a) => a.id === "memory")?.level).toBe("medium");
    expect(advise(sample({ processes: procs(250) }), calm).find((a) => a.id === "memory")?.level).toBe("high");
  });

  it("does not talk about a mini player window on a phone, which has none", () => {
    expect(advise(sample(), { ...calm, overlayEnabled: true }).some((a) => a.id === "overlay")).toBe(true);
    expect(advise(sample(), { ...calm, overlayEnabled: true }, { phone: true }).some((a) => a.id === "overlay")).toBe(false);
  });

  it("writes a report to send", () => {
    const text = report(sample(), []);
    expect(text).toContain("evplayer.exe");
    expect(text).toContain("80 МБ");
  });
});
