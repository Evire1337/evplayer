import type { JamControlMode, JamParticipant, JamSyncPayload } from "./types";
import type { PlayerState, PlayerStatus } from "@/player/store";
import type { Track } from "@/types";

const CODE_CHARS = "23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/**
 * Generate a friendly, readable room code (e.g. JAM-7K9P)
 */
export function generateRoomCode(): string {
  let suffix = "";
  for (let i = 0; i < 4; i++) {
    suffix += CODE_CHARS[Math.floor(Math.random() * CODE_CHARS.length)];
  }
  return `JAM-${suffix}`;
}

/**
 * Normalize input code (e.g. " jam - 7k9p " -> "JAM-7K9P")
 */
export function normalizeRoomCode(input: string): string {
  const cleaned = input.toUpperCase().replace(/[^A-Z0-9]/g, "");
  if (cleaned.startsWith("JAM") && cleaned.length > 3) {
    return `JAM-${cleaned.slice(3)}`;
  }
  if (cleaned.length <= 4) {
    return `JAM-${cleaned}`;
  }
  return cleaned;
}

/**
 * Calculate drift between local playback position and remote host position.
 * Takes network latency and playback progression into account.
 */
export function calculateTargetPosition(
  remotePositionMs: number,
  remoteTimestampMs: number,
  nowMs: number,
  status: PlayerStatus,
  durationMs?: number,
): number {
  if (status !== "playing") {
    return Math.max(0, remotePositionMs);
  }
  const elapsed = Math.max(0, nowMs - remoteTimestampMs);
  const target = remotePositionMs + elapsed;
  if (durationMs && durationMs > 0) {
    return Math.min(durationMs, Math.max(0, target));
  }
  return Math.max(0, target);
}

/**
 * Returns true if the difference between local and target position is high enough
 * to require a seek resync (to prevent audio jitter for minor drift).
 */
export function shouldResyncPosition(
  localMs: number,
  targetMs: number,
  thresholdMs = 1500,
): boolean {
  return Math.abs(localMs - targetMs) > thresholdMs;
}

/**
 * Build a snapshot of the current player state to sync across the network
 */
export function buildSyncPayload(
  playerState: PlayerState,
  controlMode: JamControlMode,
  participants: JamParticipant[],
  hostName: string,
  roomCode: string,
  track: Track | null,
  now = Date.now(),
): JamSyncPayload {
  return {
    track,
    queue: playerState.queue,
    index: playerState.index,
    status: playerState.status,
    positionMs: playerState.positionMs,
    timestamp: now,
    controlMode,
    participants,
    hostName,
    roomCode,
  };
}
