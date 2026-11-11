import type { Track } from "@/types";

/** who may steer the music: everyone in the session, or only the one who started it */
export type JamControl = "anyone" | "host";

export interface JamPerson {
  /** their key in the session (a fresh one each time the app starts) */
  id: string;
  name: string;
  color: string;
  host: boolean;
}

export interface JamChat {
  id: string;
  from: string;
  text: string;
  at: number;
}

export interface JamReaction {
  id: string;
  from: string;
  emoji: string;
  at: number;
}

/** who gets in with the code: anyone, whoever the host lets in, or nobody new */
export type JamAccess = "open" | "ask" | "closed";

/** someone who asked to come in and waits for the host's answer */
export interface JamKnock {
  id: string;
  name: string;
  color: string;
  at: number;
}

/** a session in the public list */
export interface JamListed {
  code: string;
  host: string;
  people: number;
  access: JamAccess;
  /** the song playing there */
  song?: string;
  seen: number;
}

/** what is playing at the host, as of `at` (the host's clock) */
export interface JamNow {
  index: number;
  trackId: string | null;
  playing: boolean;
  positionMs: number;
  at: number;
}

/** a part of the host's queue around the current song: `tracks[0]` is queue position `offset` */
export interface JamQueue {
  rev: number;
  offset: number;
  tracks: Track[];
  /** who added which of these songs, by track id */
  by?: Record<string, string>;
}

/** what a guest may ask the host to do with the music */
export type JamRequest =
  | { do: "toggle"; playing: boolean }
  | { do: "next" }
  | { do: "prev" }
  | { do: "seek"; ms: number }
  | { do: "jump"; index: number; trackId: string }
  | { do: "play"; tracks: Track[]; startIndex: number }
  | { do: "add"; track: Track; next: boolean }
  | { do: "remove"; index: number; trackId: string }
  | { do: "move"; from: number; to: number }
  | { do: "clear" };

/** everything said in a session; who said it is the signed key of the event, never the text */
export type JamMessage =
  // guest → host
  /** `ticket`: the host's pass from a Discord invite (no waiting to be let in) */
  | { t: "join"; name: string; color: string; ticket?: string }
  | { t: "here" }
  | { t: "leave" }
  | { t: "ask"; req: JamRequest }
  /** the queue this guest has is behind: send it again */
  | { t: "resync" }
  // host → everyone
  | { t: "state"; code: string; control: JamControl; access: JamAccess; people: JamPerson[]; now: JamNow; queue?: JamQueue; rev: number }
  | { t: "refuse"; to: string; reason: string }
  /** the host was asked and has not answered yet */
  | { t: "wait"; to: string }
  | { t: "kick"; id: string }
  /** the session moved to a new code: sealed for each person still in it */
  | { t: "moved"; to: string; sealed: string }
  | { t: "end" }
  /** something everyone should see (who added which song); `by` sees it already */
  | { t: "note"; text: string; by?: string }
  // anyone in the session
  | { t: "chat"; id: string; text: string }
  | { t: "react"; id: string; emoji: string }
  // the public list (a room of its own)
  | { t: "open"; code: string; host: string; people: number; access: JamAccess; song?: string }
  | { t: "gone"; code: string };

/** public relays the session talks through (Nostr, short-lived events that are not stored);
    the same list is allowed in connect-src in src-tauri/tauri.conf.json */
export const JAM_RELAYS = ["wss://relay.primal.net", "wss://relay.snort.social", "wss://offchain.pub", "wss://nos.lol"];

/** at most this many people in one session (Discord shows it as "2 из 16") */
export const JAM_MAX = 16;

export const JAM_COLORS = ["#6ff0c4", "#ff9a3c", "#3b82f6", "#8b5cf6", "#ec4899", "#10b981", "#06b6d4", "#ef4444", "#f59e0b", "#a855f7"];

export const JAM_REACTIONS = ["🔥", "❤️", "👏", "🎉", "🚀", "😭", "⚡", "🎧"];
