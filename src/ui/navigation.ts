import { create } from "zustand";

export type Page = "home" | "search" | "library";

export const useNavigation = create<{ page: Page; go: (page: Page) => void }>((set) => ({
  page: "home",
  go: (page) => set({ page }),
}));
