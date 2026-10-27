import { create } from "zustand";
import type { Track } from "@/types";

export const useStatsDialog = create<{ track: Track | null }>(() => ({ track: null }));
export const openTrackStats = (track: Track) => useStatsDialog.setState({ track });
export const closeTrackStats = () => useStatsDialog.setState({ track: null });
