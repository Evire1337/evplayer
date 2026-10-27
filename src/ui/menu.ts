import { create } from "zustand";
import type { ReactNode } from "react";

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  danger?: boolean;
  disabled?: boolean;
  onSelect?: () => void;
  /** a nested list opens to the side */
  children?: MenuItem[];
}

interface MenuState {
  open: { x: number; y: number; items: MenuItem[] } | null;
}

export const useMenu = create<MenuState>(() => ({ open: null }));

export function openMenu(event: { clientX: number; clientY: number; preventDefault?: () => void; stopPropagation?: () => void }, items: MenuItem[]): void {
  event.preventDefault?.();
  event.stopPropagation?.();
  useMenu.setState({ open: { x: event.clientX, y: event.clientY, items } });
}

export const closeMenu = () => useMenu.setState({ open: null });
