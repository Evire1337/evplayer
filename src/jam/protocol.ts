import type { Track } from "@/types";
import type { JamMessage, JamNow, JamQueue } from "./types";

// The plain rules of a session, kept apart from the network so they can be tested: codes,
// where the host's song is right now, and how much of the queue fits in one message.

const CODE_CHARS = "23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
const CODE_LENGTH = 6;
export const LINK_PREFIX = "evplayer://jam/";

/** a code that is easy to read out (no 0/O, 1/I): JAM-7K9P2Q */
export function newCode(random: () => number = Math.random): string {
  let code = "";
  for (let i = 0; i < CODE_LENGTH; i++) code += CODE_CHARS[Math.floor(random() * CODE_CHARS.length)];
  return `JAM-${code}`;
}

/** a typed or pasted code, or an invite link, as a code ("" when it is not one) */
export function normalizeCode(input: string): string {
  const raw = input.trim().replace(/^evplayer:\/\/jam\/?/i, "");
  const chars = raw.toUpperCase().replace(/[^A-Z0-9]/g, "").replace(/^JAM/, "");
  return chars.length === CODE_LENGTH ? `JAM-${chars}` : "";
}

export const inviteLink = (code: string) => `${LINK_PREFIX}${code}`;

async function sha256(text: string): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text)));
}

const hex = (bytes: Uint8Array) => Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");

/**
 * The relay tag and the key of a session, both made from its code. The relays (and anyone
 * reading them) see neither the code nor what is said: only people who know the code can.
 */
export async function roomSecrets(code: string): Promise<{ tag: string; key: Uint8Array }> {
  const [tag, key] = await Promise.all([sha256(`evplayer-jam-tag:${code}`), sha256(`evplayer-jam-key:${code}`)]);
  return { tag: hex(tag).slice(0, 32), key };
}

/** where the host's song is now, by their report from `now.at` */
export function positionNow(now: JamNow, clock = Date.now(), durationMs = 0): number {
  const ms = now.playing ? now.positionMs + Math.max(0, clock - now.at) : now.positionMs;
  return Math.max(0, durationMs > 0 ? Math.min(durationMs, ms) : ms);
}

/** only a jump this big is worth a seek (small drift is not heard; a seek is) */
export const DRIFT_MS = 1500;

/** one message is at most this big (the encryption limit is 64 KB, before base64) */
const MAX_MESSAGE = 48_000;

/**
 * A part of the queue around `index`: a few songs back and the next ones, as many as fit
 * in a message. A long queue is never sent whole.
 */
export function queueWindow(queue: Track[], index: number, rev: number, ahead = 60): JamQueue {
  for (let n = ahead; ; n = Math.floor(n / 2)) {
    const offset = Math.max(0, index - Math.min(5, n));
    const part: JamQueue = { rev, offset, tracks: queue.slice(offset, index + 1 + n) };
    if (n <= 1 || JSON.stringify(part).length <= MAX_MESSAGE) return part;
  }
}

/** a list of songs a guest asks to play, cut to what fits in a message */
export function fitTracks(tracks: Track[], startIndex: number): { tracks: Track[]; startIndex: number } {
  const part = queueWindow(tracks, startIndex, 0, 40);
  return { tracks: part.tracks, startIndex: startIndex - part.offset };
}

/** names count as the same regardless of case and spaces around */
export const sameName = (a: string, b: string) => a.trim().toLocaleLowerCase() === b.trim().toLocaleLowerCase();

const KINDS = new Set(["join", "here", "leave", "ask", "state", "refuse", "kick", "moved", "end", "chat", "react"]);

/** a message as read off a relay, or null when it is not one of ours */
export function readMessage(text: string): JamMessage | null {
  try {
    const msg = JSON.parse(text) as JamMessage;
    return msg && typeof msg === "object" && KINDS.has(msg.t) ? msg : null;
  } catch {
    return null;
  }
}
