import { create } from "zustand";

export interface Toast {
  id: number;
  text: string;
  kind: "ok" | "error";
}

let next = 1;

export const useToasts = create<{ toasts: Toast[] }>(() => ({ toasts: [] }));

export function toast(text: string, kind: Toast["kind"] = "ok"): void {
  const id = next++;
  useToasts.setState((s) => ({ toasts: [...s.toasts.slice(-2), { id, text, kind }] }));
  setTimeout(() => useToasts.setState((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), 3200);
}
