import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { closeMenu, useMenu, type MenuItem } from "../menu";

const MARGIN = 10;

function List({ items, x, y, altX, level }: { items: MenuItem[]; x: number; y: number; altX?: number; level: number }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });
  const [sub, setSub] = useState<{ index: number; x: number; y: number; altX: number } | null>(null);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    // keep the menu on screen: flip left/up when it would run off an edge
    let left = x;
    let top = y;
    if (left + r.width > window.innerWidth - MARGIN) left = altX !== undefined ? altX - r.width : window.innerWidth - r.width - MARGIN;
    if (top + r.height > window.innerHeight - MARGIN) top = Math.max(MARGIN, window.innerHeight - r.height - MARGIN);
    setPos({ left: Math.max(MARGIN, left), top });
  }, [x, y, altX]);

  return (
    <>
      <div ref={ref} className="menu" role="menu" style={{ left: pos.left, top: pos.top }}>
        {items.map((item, i) => (
          <button
            key={i}
            type="button"
            role="menuitem"
            className={`menu-item${item.danger ? " is-danger" : ""}${sub?.index === i ? " is-open" : ""}`}
            disabled={item.disabled}
            onMouseEnter={(e) => {
              if (item.children) {
                const r = e.currentTarget.getBoundingClientRect();
                setSub({ index: i, x: r.right + 4, y: r.top - 6, altX: r.left - 4 });
              } else setSub(null);
            }}
            onClick={(e) => {
              if (item.children) {
                const r = e.currentTarget.getBoundingClientRect();
                setSub({ index: i, x: r.right + 4, y: r.top - 6, altX: r.left - 4 });
                return;
              }
              closeMenu();
              item.onSelect?.();
            }}
          >
            {item.icon && <span className="menu-icon">{item.icon}</span>}
            <span className="menu-label">{item.label}</span>
            {item.children && <span className="menu-more" aria-hidden="true">›</span>}
          </button>
        ))}
      </div>
      {sub && items[sub.index]?.children && <List items={items[sub.index].children!} x={sub.x} y={sub.y} altX={sub.altX} level={level + 1} />}
    </>
  );
}

export default function ContextMenu() {
  const open = useMenu((s) => s.open);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (!(e.target as HTMLElement).closest(".menu")) closeMenu();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && closeMenu();
    const onScroll = () => closeMenu();
    document.addEventListener("pointerdown", onDown);
    document.addEventListener("keydown", onKey);
    window.addEventListener("blur", onScroll);
    return () => {
      document.removeEventListener("pointerdown", onDown);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onScroll);
    };
  }, [open]);
  if (!open) return null;
  return createPortal(<List items={open.items} x={open.x} y={open.y} level={0} />, document.body);
}
