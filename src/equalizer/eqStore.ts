import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { clamp, defaultBands, MAX_FREQ, MAX_GAIN, MAX_Q, MIN_FREQ, MIN_Q, presetBands, type EqBand, type PresetId } from "./eqModel";

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
  /** extra output level in dB on top of the automatic clip protection */
  preamp: number;
  bands: EqBand[];
  /** built-in preset id, "custom" for unsaved changes, or the id of a saved preset */
  preset: PresetId | "custom" | string;
  customPresets: CustomPreset[];
  savePreset: (name: string) => CustomPreset;
  overwritePreset: (id: string) => void;
  renamePreset: (id: string, name: string) => void;
  deletePreset: (id: string) => void;
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
      preamp: 0,
      bands: defaultBands(),
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
      deletePreset: (id) =>
        set((state) => ({
          customPresets: state.customPresets.filter((p) => p.id !== id),
          preset: state.preset === id ? "custom" : state.preset,
        })),
      applyCustomPreset: (id) =>
        set((state) => {
          const found = state.customPresets.find((p) => p.id === id);
          return found
            ? { bands: found.bands.map((band) => ({ ...band })), preamp: found.preamp, preset: id, enabled: true }
            : {};
        }),
      importPreset: (preset) => {
        const created: CustomPreset = { ...preset, id: `cp_${Date.now().toString(36)}` };
        set((state) => ({
          customPresets: [...state.customPresets, created],
          bands: created.bands.map((band) => ({ ...band })),
          preamp: created.preamp,
          preset: created.id,
          enabled: true,
        }));
        return created;
      },
      setEnabled: (enabled) => set({ enabled }),
      setPreamp: (db) => set({ preamp: clamp(db, -12, 6) }),
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
      applyPreset: (id) => set({ bands: presetBands(id), preset: id, enabled: true }),
      reset: () => set({ bands: defaultBands(), preset: "flat", preamp: 0 }),
    }),
    {
      name: "evplayer.equalizer.v1",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({ enabled: state.enabled, normalize: state.normalize, preamp: state.preamp, bands: state.bands, preset: state.preset, customPresets: state.customPresets }),
    },
  ),
);
