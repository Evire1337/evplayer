import { useEffect, useRef, useState, type ReactNode } from "react";

// a small panel that opens above its button and closes on Escape or a click elsewhere
export default function Popover({
  label,
  icon,
  children,
  width = 320,
  align = "center",
  active = false,
}: {
  label: string;
  icon: ReactNode;
  children: ReactNode;
  width?: number;
  align?: "center" | "start" | "end";
  active?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="popover-anchor" ref={ref}>
      <button
        type="button"
        className={`tool-btn${open || active ? " is-on" : ""}`}
        aria-label={label}
        aria-expanded={open}
        title={label}
        onClick={() => setOpen((v) => !v)}
      >
        {icon}
      </button>
      {open && (
        <div className={`popover glass popover-${align}`} style={{ width }} role="dialog" aria-label={label}>
          {children}
        </div>
      )}
    </div>
  );
}
