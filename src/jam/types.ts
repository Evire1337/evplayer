import type { Track } from "@/types";
import type { PlayerStatus } from "@/player/store";

export type JamControlMode = "anyone" | "host_only";

export interface JamParticipant {
  id: string;
  name: string;
  avatarColor: string;
  isHost: boolean;
  joinedAt: number;
}

export interface JamReaction {
  id: string;
  emoji: string;
  fromName: string;
  timestamp: number;
}

export interface JamChatMessage {
  id: string;
  text: string;
  fromName: string;
  timestamp: number;
}

export interface JamSyncPayload {
  track: Track | null;
  queue: Track[];
  index: number;
  status: PlayerStatus;
  positionMs: number;
  timestamp: number;
  controlMode: JamControlMode;
  participants: JamParticipant[];
  hostName: string;
  roomCode: string;
}

export type JamMessageContent =
  | { type: "join_request"; name: string; avatarColor: string }
  | { type: "join_response"; targetId: string; ok: boolean; state?: JamSyncPayload; error?: string }
  | { type: "sync"; state: JamSyncPayload }
  | { type: "track_change"; track: Track; index: number; positionMs: number; status: PlayerStatus; timestamp: number }
  | { type: "playback_state"; status: PlayerStatus; positionMs: number; timestamp: number }
  | { type: "seek"; positionMs: number; timestamp: number }
  | { type: "queue_update"; queue: Track[]; index: number }
  | { type: "queue_add"; track: Track; addedBy: string; playNext?: boolean }
  | { type: "queue_remove"; index: number }
  | { type: "queue_move"; from: number; to: number }
  | { type: "reaction"; emoji: string; fromName: string; id: string; timestamp: number }
  | { type: "chat"; text: string; fromName: string; id: string; timestamp: number }
  | { type: "control_mode"; mode: JamControlMode }
  | { type: "kick"; participantId: string; reason?: string }
  | { type: "leave"; participantId: string }
  | { type: "session_ended"; reason?: string };

export type JamMessage = JamMessageContent & { senderId: string };

export const DEFAULT_JAM_RELAYS = [
  "wss://relay.primal.net",
  "wss://nostr.wine",
];

export const JAM_AVATAR_COLORS = [
  "#1db954", // Spotify green
  "#ff5500", // SoundCloud orange
  "#3b82f6", // Blue
  "#8b5cf6", // Violet
  "#ec4899", // Pink
  "#10b981", // Emerald
  "#f59e0b", // Amber
  "#06b6d4", // Cyan
  "#ef4444", // Red
  "#a855f7", // Purple
];

export const JAM_REACTIONS = ["🔥", "❤️", "👏", "🎉", "🚀", "😭", "⚡", "🎧"];
