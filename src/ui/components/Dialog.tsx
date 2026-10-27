import { useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";

export default function Dialog({ title, open, onClose, children, wide = false }: { title: string; open: boolean; onClose: () => void; children: ReactNode; wide?: boolean }) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  if (!open) return null;
  return createPortal(
    <div className="dialog-backdrop" onPointerDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`dialog${wide ? " is-wide" : ""}`} role="dialog" aria-modal="true" aria-label={title}>
        <h2 className="dialog-title">{title}</h2>
        {children}
      </div>
    </div>,
    document.body,
  );
}
