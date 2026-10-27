import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { IconBack } from "./Icons";

// A horizontal row of cards. Arrows appear at the sides while there is more to see; a
// trackpad, Shift + wheel and dragging the scrollbar scroll it too.
export default function RowScroll({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const [edges, setEdges] = useState({ left: false, right: false });

  const measure = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    setEdges({ left: el.scrollLeft > 4, right: el.scrollLeft + el.clientWidth < el.scrollWidth - 4 });
  }, []);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    for (const child of Array.from(el.children)) observer.observe(child);
    return () => observer.disconnect();
  }, [measure, children]);

  const page = (dir: 1 | -1) => {
    const el = ref.current;
    if (el) el.scrollBy({ left: dir * el.clientWidth * 0.85, behavior: "smooth" });
  };

  return (
    <div className="row-scroll-wrap">
      {edges.left && (
        <button type="button" className="row-arrow is-left" aria-label="Назад" onClick={() => page(-1)}>
          <IconBack size={18} />
        </button>
      )}
      <div
        ref={ref}
        className="row-scroll"
        onScroll={measure}
        onWheel={(e) => {
          if (e.shiftKey && e.deltaX === 0 && ref.current) ref.current.scrollLeft += e.deltaY;
        }}
      >
        {children}
      </div>
      {edges.right && (
        <button type="button" className="row-arrow is-right" aria-label="Дальше" onClick={() => page(1)}>
          <IconBack size={18} />
        </button>
      )}
    </div>
  );
}
