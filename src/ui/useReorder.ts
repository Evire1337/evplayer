import { useCallback, useEffect, useRef, useState } from "react";
import type React from "react";

// Drag to reorder with pointer events. The browser's own drag and drop cannot be used: the
// desktop window takes drops itself (to receive files from the file manager), and it would
// not work with touch on a phone either. A press that moves less than a few pixels stays a
// normal click.

export interface ReorderState {
  from: number;
  over: number;
  before: boolean;
}

const THRESHOLD = 6;

/** where the dragged item ends up in the list after the move */
export function finalIndex(from: number, over: number, before: boolean): number {
  const to = over + (before ? 0 : 1);
  return from < to ? to - 1 : to;
}

/** `axis: "x"` for items in a row (chips), "y" for lists */
export function useReorder(onMove: (from: number, to: number) => void, axis: "x" | "y" = "y") {
  const [drag, setDrag] = useState<ReorderState | null>(null);
  const press = useRef<{ from: number; x: number; y: number; started: boolean } | null>(null);
  const current = useRef<ReorderState | null>(null);
  const moveRef = useRef(onMove);
  moveRef.current = onMove;
  const cleanup = useRef<(() => void) | null>(null);
  useEffect(() => () => cleanup.current?.(), []);

  const start = useCallback((index: number, e: React.PointerEvent) => {
    if (e.button !== 0) return;
    press.current = { from: index, x: e.clientX, y: e.clientY, started: false };
    const move = (ev: PointerEvent) => {
      const p = press.current;
      if (!p) return;
      if (!p.started) {
        if (Math.hypot(ev.clientX - p.x, ev.clientY - p.y) < THRESHOLD) return;
        p.started = true;
        document.documentElement.classList.add("is-reordering");
      }
      const row = document.elementFromPoint(ev.clientX, ev.clientY)?.closest<HTMLElement>("[data-reorder]");
      if (!row) return;
      const r = row.getBoundingClientRect();
      const before = axis === "x" ? ev.clientX < r.left + r.width / 2 : ev.clientY < r.top + r.height / 2;
      const next = { from: p.from, over: Number(row.dataset.reorder), before };
      const c = current.current;
      if (!c || c.over !== next.over || c.before !== next.before) {
        current.current = next;
        setDrag(next);
      }
    };
    const up = () => {
      const p = press.current;
      const c = current.current;
      finish();
      if (!p?.started) return;
      // the press was a drag: the click that follows must not play the track
      const swallow = (ev: MouseEvent) => {
        ev.stopPropagation();
        ev.preventDefault();
      };
      window.addEventListener("click", swallow, { capture: true });
      setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
      if (c && c.over !== c.from) {
        const to = finalIndex(c.from, c.over, c.before);
        if (to !== c.from) moveRef.current(c.from, to);
      }
    };
    const cancel = (ev: KeyboardEvent) => ev.key === "Escape" && finish();
    const finish = () => {
      press.current = null;
      current.current = null;
      setDrag(null);
      document.documentElement.classList.remove("is-reordering");
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", finish);
      window.removeEventListener("keydown", cancel);
      cleanup.current = null;
    };
    cleanup.current?.();
    cleanup.current = finish;
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", finish);
    window.addEventListener("keydown", cancel);
  }, [axis]);

  return {
    dragging: drag !== null,
    /** spread on each row */
    rowProps: (index: number) => ({ "data-reorder": index, onPointerDown: (e: React.PointerEvent) => start(index, e) }),
    hint: (index: number): "before" | "after" | null => (drag && drag.over === index && drag.from !== index ? (drag.before ? "before" : "after") : null),
  };
}
