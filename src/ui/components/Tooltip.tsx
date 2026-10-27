import { useEffect, useState } from "react";
import { createPortal } from "react-dom";

// Hover hints in the style of the menus: any element with data-tip="…" gets one.
// One listener for the whole app, so buttons stay plain.

interface Tip {
  text: string;
  x: number;
  y: number;
  below: boolean;
}

const DELAY_MS = 350;

export default function Tooltip() {
  const [tip, setTip] = useState<Tip | null>(null);
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    let current: HTMLElement | null = null;
    const hide = () => {
      if (timer) clearTimeout(timer);
      timer = null;
      current = null;
      setTip(null);
    };
    const onOver = (e: PointerEvent) => {
      const el = (e.target as HTMLElement | null)?.closest?.<HTMLElement>("[data-tip]") ?? null;
      if (el === current) return;
      hide();
      if (!el || !el.dataset.tip) return;
      current = el;
      timer = setTimeout(() => {
        if (!current || !document.contains(current)) return;
        const r = current.getBoundingClientRect();
        const below = r.top < 70;
        setTip({ text: current.dataset.tip!, x: r.left + r.width / 2, y: below ? r.bottom + 8 : r.top - 8, below });
      }, DELAY_MS);
    };
    document.addEventListener("pointerover", onOver);
    document.addEventListener("pointerdown", hide, true);
    window.addEventListener("blur", hide);
    document.addEventListener("scroll", hide, true);
    return () => {
      document.removeEventListener("pointerover", onOver);
      document.removeEventListener("pointerdown", hide, true);
      window.removeEventListener("blur", hide);
      document.removeEventListener("scroll", hide, true);
      if (timer) clearTimeout(timer);
    };
  }, []);
  if (!tip) return null;
  const left = Math.min(Math.max(tip.x, 90), window.innerWidth - 90);
  return createPortal(
    <div className={`tooltip${tip.below ? " is-below" : ""}`} role="tooltip" style={{ left, top: tip.y }}>
      {tip.text}
    </div>,
    document.body,
  );
}
