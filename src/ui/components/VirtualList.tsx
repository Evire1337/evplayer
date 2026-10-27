import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";

// Long lists draw only the rows that are on screen (plus a margin). A playlist of a
// thousand songs then costs about as much memory and time as one of thirty.

const OVERSCAN = 12;

function scrollParent(el: HTMLElement | null): HTMLElement | null {
  for (let node = el?.parentElement ?? null; node; node = node.parentElement) {
    const style = getComputedStyle(node);
    if (/(auto|scroll)/.test(style.overflowY)) return node;
  }
  return null;
}

export default function VirtualList<T>({
  items,
  render,
  getKey,
  threshold = 60,
  estimate = 64,
}: {
  items: T[];
  render: (item: T, index: number) => ReactNode;
  getKey: (item: T, index: number) => string;
  /** shorter lists are drawn whole */
  threshold?: number;
  /** row height with the gap, until a real row is measured */
  estimate?: number;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [rowH, setRowH] = useState(estimate);
  const [range, setRange] = useState({ start: 0, end: Math.min(items.length, 40) });
  const virtual = items.length >= threshold;

  useEffect(() => {
    if (!virtual) return;
    const el = ref.current;
    const scroller = scrollParent(el);
    if (!el || !scroller) return;
    let frame = 0;
    const update = () => {
      frame = 0;
      const listTop = el.getBoundingClientRect().top - scroller.getBoundingClientRect().top;
      const first = Math.floor(-listTop / rowH);
      const count = Math.ceil(scroller.clientHeight / rowH);
      const start = Math.max(0, first - OVERSCAN);
      const end = Math.min(items.length, first + count + OVERSCAN);
      setRange((r) => (r.start === start && r.end === end ? r : { start, end }));
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };
    update();
    scroller.addEventListener("scroll", schedule, { passive: true });
    const observer = new ResizeObserver(schedule);
    observer.observe(scroller);
    return () => {
      scroller.removeEventListener("scroll", schedule);
      observer.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, [virtual, items.length, rowH]);

  // measure a real row once: the distance between two rows includes the gap
  useLayoutEffect(() => {
    if (!virtual || !ref.current) return;
    const rows = ref.current.children;
    if (rows.length >= 2) {
      const h = (rows[1] as HTMLElement).offsetTop - (rows[0] as HTMLElement).offsetTop;
      if (h > 20 && Math.abs(h - rowH) > 0.5) setRowH(h);
    }
  });

  if (!virtual) return <>{items.map((item, i) => <Keyed key={getKey(item, i)}>{render(item, i)}</Keyed>)}</>;
  const start = Math.min(range.start, items.length);
  const end = Math.min(Math.max(range.end, start), items.length);
  return (
    <div ref={ref} className="virtual-list" style={{ paddingTop: start * rowH, paddingBottom: (items.length - end) * rowH }}>
      {items.slice(start, end).map((item, i) => (
        <Keyed key={getKey(item, start + i)}>{render(item, start + i)}</Keyed>
      ))}
    </div>
  );
}

function Keyed({ children }: { children: ReactNode }) {
  return <>{children}</>;
}
