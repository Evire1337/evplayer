import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { DiscordUser } from "@/platform/api";
import { JAM_COLORS, type JamChat, type JamControl, type JamPerson, type JamReaction } from "./types";

// What the page shows of a listening session. The engine (./engine) writes it; the dialog,
// the banner and the queue read it. The name and colour are remembered on this device.

export type JamStatus = "idle" | "starting" | "joining" | "live";

interface JamState {
  open: boolean;
  status: JamStatus;
  host: boolean;
  code: string;
  control: JamControl;
  people: JamPerson[];
  chat: JamChat[];
  reactions: JamReaction[];
  /** who put a song in the queue, by track id */
  addedBy: Record<string, string>;
  /** friends who pressed "Ask to join" in Discord, waiting for an answer */
  asks: DiscordUser[];
  name: string;
  color: string;
  setOpen: (open: boolean) => void;
  setProfile: (patch: Partial<Pick<JamState, "name" | "color">>) => void;
}

const CHAT_KEPT = 50;
const REACTIONS_KEPT = 20;

export const useJam = create<JamState>()(
  persist(
    (set) => ({
      open: false,
      status: "idle",
      host: false,
      code: "",
      control: "anyone",
      people: [],
      chat: [],
      reactions: [],
      addedBy: {},
      asks: [],
      name: "",
      color: JAM_COLORS[Math.floor(Math.random() * JAM_COLORS.length)],
      setOpen: (open) => set({ open }),
      setProfile: (patch) => set({ ...patch, ...(patch.name !== undefined && { name: patch.name.slice(0, 24) }) }),
    }),
    { name: "evplayer.jam.v1", storage: createJSONStorage(() => localStorage), partialize: (s) => ({ name: s.name, color: s.color }) },
  ),
);

export const jamLive = () => useJam.getState().status === "live";

export function addChat(msg: JamChat): void {
  useJam.setState((s) => ({ chat: [...s.chat.slice(1 - CHAT_KEPT), msg] }));
}

export function addReaction(r: JamReaction): void {
  useJam.setState((s) => ({ reactions: [...s.reactions.slice(1 - REACTIONS_KEPT), r] }));
}

export function noteAddedBy(trackId: string, name: string): void {
  useJam.setState((s) => ({ addedBy: { ...s.addedBy, [trackId]: name } }));
}

/** the session is over here: back to the start screen (the name and colour stay) */
export function resetJam(): void {
  useJam.setState({ status: "idle", host: false, code: "", control: "anyone", people: [], chat: [], reactions: [], addedBy: {} });
}

/** the name a key goes by in the session ("Кто-то" for a key that is not in it) */
export function nameOf(id: string): string {
  return useJam.getState().people.find((p) => p.id === id)?.name ?? "Кто-то";
}
