import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { bandsForMode, clamp, MAX_FREQ, MAX_GAIN, MAX_PREAMP, MIN_PREAMP, MAX_Q, MIN_FREQ, MIN_Q, modeOf, presetBands, type EqBand, type EqMode, type PresetId } from "./eqModel";

export interface CustomPreset {
  id: string;
  name: string;
  bands: EqBand[];
  preamp: number;
}

interface EqState {
  enabled: boolean;
  /** keep every song at about the same loudness */
  normalize: boolean;
  setNormalize: (normalize: boolean) => void;
  /**
   * clip protection: the level is pulled down by most of the boost and a limiter catches
   * the rest. Off by default: boosts come through at full strength, loud ones may distort
   */
  protect: boolean;
  setProtect: (protect: boolean) => void;
  /** extra output level in dB on top of the automatic clip protection */
  preamp: number;
  /** six movable bands, or ten on the classic octaves */
  mode: EqMode;
  setMode: (mode: EqMode) => void;
  /** the bands and the preset of the mode not in use, kept for switching back */
  otherBands: EqBand[];
  otherPreset: string;
  bands: EqBand[];
  /** built-in preset id, "custom" for unsaved changes, or the id of a saved preset */
  preset: PresetId | "custom" | string;
  customPresets: CustomPreset[];
  savePreset: (name: string) => CustomPreset;
  overwritePreset: (id: string) => void;
  renamePreset: (id: string, name: string) => void;
  deletePreset: (id: string) => void;
  /** reorder the saved presets of one mode (positions within that mode's list) */
  movePreset: (from: number, to: number, mode: EqMode) => void;
  applyCustomPreset: (id: string) => void;
  importPreset: (preset: Omit<CustomPreset, "id">) => CustomPreset;
  setEnabled: (enabled: boolean) => void;
  setPreamp: (db: number) => void;
  updateBand: (id: string, patch: Partial<Omit<EqBand, "id">>) => void;
  applyPreset: (id: PresetId) => void;
  reset: () => void;
}

export const useEqualizerStore = create<EqState>()(
  persist(
    (set) => ({
      enabled: false,
      normalize: true,
      setNormalize: (normalize) => set({ normalize }),
      protect: false,
      setProtect: (protect) => set({ protect }),
      preamp: 0,
      mode: "simple",
      otherBands: bandsForMode("advanced"),
      otherPreset: "flat",
      // each mode keeps its own bands and its own chosen preset; switching swaps them
      setMode: (mode) =>
        set((state) => {
          if (mode === state.mode) return {};
          const bands = state.otherBands.length && modeOf(state.otherBands) === mode ? state.otherBands : bandsForMode(mode);
          return { mode, bands, preset: state.otherPreset || "flat", otherBands: state.bands, otherPreset: state.preset };
        }),
      bands: bandsForMode("simple"),
      preset: "flat",
      customPresets: [],
      savePreset: (name) => {
        const state = useEqualizerStore.getState();
        const created: CustomPreset = {
          id: `cp_${Date.now().toString(36)}`,
          name: name.trim().slice(0, 32) || "Preset",
          bands: state.bands.map((band) => ({ ...band })),
          preamp: state.preamp,
        };
        set({ customPresets: [...state.customPresets, created], preset: created.id });
        return created;
      },
      overwritePreset: (id) =>
        set((state) => ({
          preset: id,
          customPresets: state.customPresets.map((p) =>
            p.id === id ? { ...p, bands: state.bands.map((band) => ({ ...band })), preamp: state.preamp } : p,
          ),
        })),
      renamePreset: (id, name) =>
        set((state) => ({
          customPresets: state.customPresets.map((p) => (p.id === id ? { ...p, name: name.trim().slice(0, 32) || p.name } : p)),
        })),
      movePreset: (from, to, mode) =>
        set((state) => {
          // reorder this mode's presets among themselves; the other mode's keep their places
          const slots = state.customPresets.map((p, i) => (modeOf(p.bands) === mode ? i : -1)).filter((i) => i >= 0);
          const own = slots.map((i) => state.customPresets[i]);
          const [item] = own.splice(from, 1);
          if (!item) return {};
          own.splice(Math.max(0, Math.min(own.length, to)), 0, item);
          const list = [...state.customPresets];
          slots.forEach((slot, k) => (list[slot] = own[k]));
          return { customPresets: list };
        }),
      deletePreset: (id) =>
        set((state) => ({
          customPresets: state.customPresets.filter((p) => p.id !== id),
          preset: state.preset === id ? "custom" : state.preset,
        })),
      applyCustomPreset: (id) =>
        set((state) => {
          const found = state.customPresets.find((p) => p.id === id);
          if (!found) return {};
          const mode = modeOf(found.bands);
          return {
            bands: found.bands.map((band) => ({ ...band })),
            preamp: found.preamp,
            preset: id,
            enabled: true,
            // a preset saved in the other mode switches the mode with it
            ...(mode !== state.mode ? { mode, otherBands: state.bands, otherPreset: state.preset } : {}),
          };
        }),
      importPreset: (preset) => {
        const created: CustomPreset = { ...preset, id: `cp_${Date.now().toString(36)}` };
        set((state) => {
          const mode = modeOf(created.bands);
          return {
            customPresets: [...state.customPresets, created],
            bands: created.bands.map((band) => ({ ...band })),
            preamp: created.preamp,
            preset: created.id,
            enabled: true,
            ...(mode !== state.mode ? { mode, otherBands: state.bands, otherPreset: state.preset } : {}),
          };
        });
        return created;
      },
      setEnabled: (enabled) => set({ enabled }),
      setPreamp: (db) => set({ preamp: clamp(db, MIN_PREAMP, MAX_PREAMP) }),
      updateBand: (id, patch) =>
        set((state) => ({
          preset: "custom",
          bands: state.bands.map((band) =>
            band.id !== id
              ? band
              : {
                  ...band,
                  ...patch,
                  frequency: clamp(patch.frequency ?? band.frequency, MIN_FREQ, MAX_FREQ),
                  gain: Math.round(clamp(patch.gain ?? band.gain, -MAX_GAIN, MAX_GAIN) * 10) / 10,
                  q: clamp(patch.q ?? band.q, MIN_Q, MAX_Q),
                },
          ),
        })),
      applyPreset: (id) => set((state) => ({ bands: presetBands(id, state.mode), preset: id, enabled: true })),
      reset: () => set((state) => ({ bands: bandsForMode(state.mode), preset: "flat", preamp: 0 })),
    }),
    {
      name: "evplayer.equalizer.v1",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        enabled: state.enabled,
        normalize: state.normalize,
        protect: state.protect,
        preamp: state.preamp,
        mode: state.mode,
        otherBands: state.otherBands,
        otherPreset: state.otherPreset,
        bands: state.bands,
        preset: state.preset,
        customPresets: state.customPresets,
      }),
    },
  ),
);
