import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { DiscordUser } from "@/platform/api";
import { JAM_COLORS, type JamAccess, type JamChat, type JamControl, type JamKnock, type JamListed, type JamPerson, type JamReaction } from "./types";

// What the page shows of a listening session. The engine (./engine) writes it; the dialog,
// the banner and the queue read it. Remembered on this device: the name and colour, how
// sessions started here are set up, and whether the public list is shown.

export type JamStatus = "idle" | "starting" | "joining" | "live";

/** how a session started here is set up (and stays set up for the next one) */
export interface JamSetup {
  control: JamControl;
  access: JamAccess;
  /** shown in the public list */
  listed: boolean;
}

interface JamState {
  open: boolean;
  status: JamStatus;
  /** joining: asked the host, waiting to be let in */
  waiting: boolean;
  host: boolean;
  code: string;
  /** the session's own settings (the host's, as it last said) */
  control: JamControl;
  access: JamAccess;
  listed: boolean;
  people: JamPerson[];
  /** the host: people who asked to come in */
  knocks: JamKnock[];
  chat: JamChat[];
  reactions: JamReaction[];
  /** who put a song in the queue, by track id */
  addedBy: Record<string, string>;
  /** friends who pressed "Ask to join" in Discord, waiting for an answer */
  asks: DiscordUser[];
  /** open sessions others put in the public list */
  lobby: JamListed[];
  setup: JamSetup;
  /** look at the public list when joining */
  browse: boolean;
  name: string;
  color: string;
  setOpen: (open: boolean) => void;
  setProfile: (patch: Partial<Pick<JamState, "name" | "color">>) => void;
  setSetup: (patch: Partial<JamSetup>) => void;
}

const CHAT_KEPT = 50;
const REACTIONS_KEPT = 20;
const DEFAULT_SETUP: JamSetup = { control: "anyone", access: "open", listed: false };

export const useJam = create<JamState>()(
  persist(
    (set) => ({
      open: false,
      status: "idle",
      waiting: false,
      host: false,
      code: "",
      ...DEFAULT_SETUP,
      people: [],
      knocks: [],
      chat: [],
      reactions: [],
      addedBy: {},
      asks: [],
      lobby: [],
      setup: DEFAULT_SETUP,
      browse: true,
      name: "",
      color: JAM_COLORS[Math.floor(Math.random() * JAM_COLORS.length)],
      setOpen: (open) => set({ open }),
      setProfile: (patch) => set({ ...patch, ...(patch.name !== undefined && { name: patch.name.slice(0, 24) }) }),
      setSetup: (patch) => set((s) => ({ setup: { ...s.setup, ...patch } })),
    }),
    {
      name: "evplayer.jam.v1",
      storage: createJSONStorage(() => localStorage),
      partialize: (s) => ({ name: s.name, color: s.color, setup: s.setup, browse: s.browse }),
    },
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

/** the session is over here: back to the start screen (the name, colour and setup stay) */
export function resetJam(): void {
  useJam.setState({ status: "idle", waiting: false, host: false, code: "", ...DEFAULT_SETUP, people: [], knocks: [], chat: [], reactions: [], addedBy: {} });
}

/** the name a key goes by in the session ("Кто-то" for a key that is not in it) */
export function nameOf(id: string): string {
  return useJam.getState().people.find((p) => p.id === id)?.name ?? "Кто-то";
}
