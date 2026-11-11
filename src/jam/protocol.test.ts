import { describe, expect, it } from "vitest";
import type { Track } from "@/types";
import { fitTracks, inviteLink, newCode, normalizeCode, positionNow, queueWindow, readMessage, roomSecrets, sameName, ticketOf } from "./protocol";

const track = (i: number, extra = 0): Track => ({
  id: `sc:${i}`,
  source: "soundcloud",
  title: `Song ${i}${"x".repeat(extra)}`,
  artists: [{ id: "a", name: "Artist" }],
  durationMs: 200_000,
});

describe("codes", () => {
  it("makes readable codes and reads them back from anything typed or pasted", () => {
    const code = newCode();
    expect(code).toMatch(/^JAM-[2-9A-HJ-NP-Z]{4}-[2-9A-HJ-NP-Z]{4}$/);
    expect(normalizeCode(code)).toBe(code);
    expect(normalizeCode(" jam - 7k9p 2qxz ")).toBe("JAM-7K9P-2QXZ");
    expect(normalizeCode("7k9p2qxz")).toBe("JAM-7K9P-2QXZ");
    expect(normalizeCode(inviteLink("JAM-7K9P-2QXZ"))).toBe("JAM-7K9P-2QXZ");
    expect(normalizeCode("JAM-7K9P-2Q")).toBe("");
    expect(normalizeCode("")).toBe("");
  });

  it("reads the host's pass from a Discord invite", () => {
    expect(normalizeCode("JAM-7K9P-2QXZ~a1b2")).toBe("JAM-7K9P-2QXZ");
    expect(ticketOf("JAM-7K9P-2QXZ~a1b2")).toBe("a1b2");
    expect(ticketOf("JAM-7K9P-2QXZ")).toBeUndefined();
  });
  it("hides the code: the tag and the key differ per code and do not contain it", async () => {
    const a = await roomSecrets("JAM-7K9P2Q");
    const b = await roomSecrets("JAM-7K9P2R");
    expect(a.tag).not.toBe(b.tag);
    expect(a.tag).not.toContain("7K9P2Q");
    expect(a.key).toHaveLength(32);
    expect(await roomSecrets("JAM-7K9P2Q")).toEqual(a);
  });
});

describe("positionNow", () => {
  const now = { index: 0, trackId: "sc:1", playing: true, positionMs: 10_000, at: 1_000 };

  it("moves on with the clock while playing, stands still while paused", () => {
    expect(positionNow(now, 4_000)).toBe(13_000);
    expect(positionNow({ ...now, playing: false }, 4_000)).toBe(10_000);
  });

  it("stays within the song, and a clock that is behind does not go back", () => {
    expect(positionNow(now, 999_000, 60_000)).toBe(60_000);
    expect(positionNow(now, 0)).toBe(10_000);
  });
});

describe("queueWindow", () => {
  const queue = Array.from({ length: 500 }, (_, i) => track(i));

  it("sends a few songs back and the next ones, not the whole queue", () => {
    const w = queueWindow(queue, 100, 7);
    expect(w.rev).toBe(7);
    expect(w.offset).toBe(95);
    expect(w.tracks[0].id).toBe("sc:95");
    expect(w.tracks[100 - w.offset].id).toBe("sc:100");
    expect(w.tracks.length).toBeLessThan(80);
  });

  it("starts at the beginning near the start of the queue", () => {
    expect(queueWindow(queue, 2, 0).offset).toBe(0);
  });

  it("takes fewer songs when they would not fit in one message", () => {
    const big = Array.from({ length: 200 }, (_, i) => track(i, 2000));
    const w = queueWindow(big, 50, 0);
    expect(JSON.stringify(w).length).toBeLessThanOrEqual(48_000);
    expect(w.tracks[50 - w.offset].id).toBe("sc:50");
  });

  it("keeps the pressed song when a long list is cut", () => {
    const fit = fitTracks(queue, 300);
    expect(fit.tracks[fit.startIndex].id).toBe("sc:300");
  });
});

describe("messages", () => {
  it("takes only messages of a session", () => {
    expect(readMessage(JSON.stringify({ t: "chat", id: "1", text: "hi" }))).toEqual({ t: "chat", id: "1", text: "hi" });
    expect(readMessage(JSON.stringify({ t: "hack" }))).toBeNull();
    expect(readMessage("not json")).toBeNull();
    expect(readMessage("null")).toBeNull();
  });

  it("treats names as the same regardless of case and spaces", () => {
    expect(sameName(" Вася ", "вася")).toBe(true);
    expect(sameName("Вася", "Вася2")).toBe(false);
  });
});
