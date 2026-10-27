import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

const MARGIN = 12;

// A small panel that opens above its button and closes on Escape or a click elsewhere.
// It is positioned against the window, so near an edge it slides inward instead of
// spilling off screen.
export default function Popover({
  label,
  icon,
  children,
  width = 320,
  active = false,
}: {
  label: string;
  icon: ReactNode;
  children: ReactNode;
  width?: number;
  active?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ left: number; bottom: number; maxHeight: number } | null>(null);
  const anchorRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const place = useCallback(() => {
    const button = buttonRef.current;
    if (!button) return;
    const rect = button.getBoundingClientRect();
    const w = Math.min(width, window.innerWidth - MARGIN * 2);
    const centred = rect.left + rect.width / 2 - w / 2;
    const left = Math.min(Math.max(MARGIN, centred), window.innerWidth - w - MARGIN);
    const bottom = window.innerHeight - rect.top + 10;
    setPos({ left, bottom, maxHeight: Math.max(160, rect.top - 10 - MARGIN - 30) });
  }, [width]);

  useLayoutEffect(() => {
    if (open) place();
  }, [open, place]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      const target = e.target as Node;
      if (anchorRef.current?.contains(target) || panelRef.current?.contains(target)) return;
      setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", onDown);
    document.addEventListener("keydown", onKey);
    window.addEventListener("resize", place);
    return () => {
      document.removeEventListener("pointerdown", onDown);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", place);
    };
  }, [open, place]);

  return (
    <div className="popover-anchor" ref={anchorRef}>
      <button
        ref={buttonRef}
        type="button"
        className={`tool-btn${open || active ? " is-on" : ""}`}
        aria-label={label}
        aria-expanded={open}
        title={label}
        onClick={() => setOpen((v) => !v)}
      >
        {icon}
      </button>
      {open && pos && createPortal(
        // rendered at the top of the page: glass panels around the button would otherwise
        // become the frame "fixed" positioning is measured against
        <div
          ref={panelRef}
          className="popover"
          style={{ width: Math.min(width, window.innerWidth - MARGIN * 2), left: pos.left, bottom: pos.bottom, maxHeight: pos.maxHeight }}
          role="dialog"
          aria-label={label}
        >
          {children}
        </div>,
        document.body,
      )}
    </div>
  );
}
