import { create } from "zustand";
import {
  JAM_AVATAR_COLORS,
  type JamChatMessage,
  type JamControlMode,
  type JamParticipant,
  type JamReaction,
} from "./types";

const PROFILE_KEY = "evplayer.jam.profile";

export interface UserProfile {
  name: string;
  avatarColor: string;
}

function randomColor(): string {
  return JAM_AVATAR_COLORS[Math.floor(Math.random() * JAM_AVATAR_COLORS.length)];
}

function loadProfile(): UserProfile {
  try {
    const raw = localStorage.getItem(PROFILE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (typeof parsed?.name === "string" && typeof parsed?.avatarColor === "string") {
        return { name: parsed.name.slice(0, 32), avatarColor: parsed.avatarColor };
      }
    }
  } catch {
    // fallback
  }
  return {
    name: "Слушатель",
    avatarColor: randomColor(),
  };
}

export interface JamStoreState {
  isOpen: boolean;
  active: boolean;
  status: "idle" | "hosting" | "joining" | "connected" | "disconnected" | "error";
  error?: string;
  isHost: boolean;
  roomCode: string;
  hostName: string;
  controlMode: JamControlMode;
  participants: JamParticipant[];
  myProfile: UserProfile;
  reactions: JamReaction[];
  chatMessages: JamChatMessage[];
  trackAddedBy: Record<string, string>;

  // actions
  setOpen: (open: boolean) => void;
  setProfile: (patch: Partial<UserProfile>) => void;
  setControlMode: (mode: JamControlMode) => void;
  setStatus: (status: JamStoreState["status"], error?: string) => void;
  setSession: (params: {
    active: boolean;
    isHost: boolean;
    roomCode: string;
    hostName: string;
    controlMode?: JamControlMode;
    participants?: JamParticipant[];
  }) => void;
  updateParticipants: (participants: JamParticipant[]) => void;
  addReaction: (reaction: JamReaction) => void;
  addChatMessage: (msg: JamChatMessage) => void;
  recordTrackAddedBy: (trackId: string, addedBy: string) => void;
  resetSession: () => void;
}

export const useJam = create<JamStoreState>((set) => ({
  isOpen: false,
  active: false,
  status: "idle",
  isHost: false,
  roomCode: "",
  hostName: "",
  controlMode: "anyone",
  participants: [],
  myProfile: loadProfile(),
  reactions: [],
  chatMessages: [],
  trackAddedBy: {},

  setOpen: (isOpen) => set({ isOpen }),

  setProfile: (patch) =>
    set((state) => {
      const myProfile = { ...state.myProfile, ...patch };
      try {
        localStorage.setItem(PROFILE_KEY, JSON.stringify(myProfile));
      } catch {
        // ignore storage errors
      }
      return { myProfile };
    }),

  setControlMode: (controlMode) => set({ controlMode }),

  setStatus: (status, error) => set({ status, error }),

  setSession: ({ active, isHost, roomCode, hostName, controlMode = "anyone", participants = [] }) =>
    set({
      active,
      isHost,
      roomCode,
      hostName,
      controlMode,
      participants,
      status: active ? "connected" : "idle",
      error: undefined,
    }),

  updateParticipants: (participants) => set({ participants }),

  addReaction: (reaction) =>
    set((state) => {
      const now = Date.now();
      const withLocalTime = {
        ...reaction,
        receivedAt: reaction.receivedAt || now,
      };
      // keep last 20 reactions, prune older than 12 seconds
      const reactions = [
        ...state.reactions.filter((r) => now - (r.receivedAt || r.timestamp) < 12000),
        withLocalTime,
      ].slice(-20);
      return { reactions };
    }),

  addChatMessage: (msg) =>
    set((state) => {
      const withLocalTime = {
        ...msg,
        receivedAt: msg.receivedAt || Date.now(),
      };
      return {
        chatMessages: [...state.chatMessages.slice(-49), withLocalTime],
      };
    }),

  recordTrackAddedBy: (trackId, addedBy) =>
    set((state) => ({
      trackAddedBy: { ...state.trackAddedBy, [trackId]: addedBy },
    })),

  resetSession: () =>
    set({
      active: false,
      isHost: false,
      roomCode: "",
      hostName: "",
      controlMode: "anyone",
      participants: [],
      status: "idle",
      error: undefined,
      reactions: [],
      chatMessages: [],
    }),
}));
