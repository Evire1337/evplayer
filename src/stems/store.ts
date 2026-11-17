import { create } from "zustand";
import type { StemsMeta } from "@/platform/api";

// What the splitter is doing, for the page: the switch's state, the model download, and each
// song's progress. Nothing here is saved: the switch itself is a setting (stemsOn).

export type StemPhase = "off" | "download" | "loading" | "ready" | "error";

export interface StemJob {
  status: "queued" | "decoding" | "separating" | "ready" | "error";
  /** 0 to 1 over the whole job */
  share: number;
  meta?: StemsMeta;
  error?: string;
}

interface StemsStore {
  phase: StemPhase;
  /** 0 to 1 while the model downloads */
  modelShare: number;
  error?: string;
  cacheBytes: number;
  jobs: Record<string, StemJob>;
  /** the key of the song whose tracks play now */
  engaged: string | null;
  /** songs whose split tracks would not start: they play as they are, and are not tried again */
  broken: Record<string, true>;
  /** bumped when something that was not ready (the sound graph) is, so the player looks again */
  tick: number;
}

export const useStems = create<StemsStore>(() => ({ phase: "off", modelShare: 0, cacheBytes: 0, jobs: {}, engaged: null, broken: {}, tick: 0 }));

export function setJob(key: string, patch: Partial<StemJob> & Pick<StemJob, "status">): void {
  useStems.setState((s) => ({ jobs: { ...s.jobs, [key]: { ...(s.jobs[key] ?? { share: 0 }), ...patch } } }));
}
