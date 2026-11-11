import { describe, expect, it } from "vitest";
import {
  calculateTargetPosition,
  generateRoomCode,
  normalizeRoomCode,
  shouldResyncPosition,
  buildSyncPayload,
} from "./protocol";
import type { PlayerState } from "@/player/store";
import type { Track } from "@/types";

describe("jam protocol", () => {
  it("generates valid room codes", () => {
    const code = generateRoomCode();
    expect(code).toMatch(/^JAM-[2-9A-Z]{4}$/);
  });

  it("normalizes room codes", () => {
    expect(normalizeRoomCode("7k9p")).toBe("JAM-7K9P");
    expect(normalizeRoomCode("jam-7k9p")).toBe("JAM-7K9P");
    expect(normalizeRoomCode("  JAM 7K9P  ")).toBe("JAM-7K9P");
    expect(normalizeRoomCode("abc123xyz")).toBe("ABC123XYZ");
  });

  it("calculates target position with playback progress compensation", () => {
    const baseMs = 10_000;
    const sentAt = 1_000_000;
    const receivedAt = 1_002_000; // 2 seconds later

    // when playing, target should advance by 2000ms
    const targetPlaying = calculateTargetPosition(baseMs, sentAt, receivedAt, "playing", 100_000);
    expect(targetPlaying).toBe(12_000);

    // when paused, target should not advance
    const targetPaused = calculateTargetPosition(baseMs, sentAt, receivedAt, "paused", 100_000);
    expect(targetPaused).toBe(10_000);

    // does not exceed duration
    const clamped = calculateTargetPosition(99_000, sentAt, receivedAt, "playing", 100_000);
    expect(clamped).toBe(100_000);
  });

  it("checks whether resync is needed based on threshold", () => {
    expect(shouldResyncPosition(10_000, 10_500, 1500)).toBe(false); // 500ms diff, no jitter seek
    expect(shouldResyncPosition(10_000, 11_400, 1500)).toBe(false); // 1400ms diff
    expect(shouldResyncPosition(10_000, 11_600, 1500)).toBe(true);  // 1600ms diff, seek needed
  });

  it("builds sync payload correctly", () => {
    const dummyTrack: Track = {
      id: "sc:123",
      source: "soundcloud",
      title: "Song",
      artists: [{ id: "1", name: "Artist" }],
      durationMs: 180_000,
    };
    const playerState: PlayerState = {
      queue: [dummyTrack],
      index: 0,
      status: "playing",
      positionMs: 30_000,
      durationMs: 180_000,
      volume: 0.8,
      muted: false,
      repeat: "off",
      shuffle: false,
    };
    const payload = buildSyncPayload(
      playerState,
      "anyone",
      [],
      "Alex",
      "JAM-1234",
      dummyTrack,
      1234567,
    );
    expect(payload.roomCode).toBe("JAM-1234");
    expect(payload.hostName).toBe("Alex");
    expect(payload.status).toBe("playing");
    expect(payload.positionMs).toBe(30_000);
    expect(payload.timestamp).toBe(1234567);
  });
});
