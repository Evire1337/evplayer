import { useEffect, useRef } from "react";
import { BackdropRenderer } from "./BackdropRenderer";
import { useVisible } from "@/app/visibility";
import { useSettings } from "@/settings/store";

export default function Backdrop({ image, playing }: { image?: string; playing: boolean }) {
  const hostRef = useRef<HTMLDivElement>(null);
  const rendererRef = useRef<BackdropRenderer | null>(null);
  const visible = useVisible((s) => s.visible);
  const animated = useSettings((s) => s.animatedBackdrop);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    // a canvas of its own for each renderer: destroy() gives the graphics context back for
    // good, and a context that was given back cannot be asked for again from the same canvas
    // (React in development mounts, unmounts and mounts again, and the second time the
    // background stayed a plain gradient)
    const canvas = document.createElement("canvas");
    canvas.className = "backdrop-canvas";
    host.prepend(canvas);
    try {
      rendererRef.current = new BackdropRenderer(canvas);
    } catch {
      rendererRef.current = null; // no WebGL: the CSS gradient behind stays visible
    }
    return () => {
      rendererRef.current?.destroy();
      rendererRef.current = null;
      canvas.remove();
    };
  }, []);

  useEffect(() => {
    const r = rendererRef.current;
    if (!r) return;
    if (!visible) r.stop();
    else if (animated) r.start();
    else r.still();
  }, [visible, animated]);

  useEffect(() => {
    const r = rendererRef.current;
    if (!r) return;
    r.setImage(image ?? null);
    if (!animated && visible) {
      // let the new cover fade in, then hold still again
      r.start();
      const timer = setTimeout(() => r.still(), 1600);
      return () => clearTimeout(timer);
    }
  }, [image, animated, visible]);

  useEffect(() => {
    const r = rendererRef.current;
    if (!r) return;
    // slower and dimmer while paused, so the screen calms down
    r.speed = playing ? 1 : 0.25;
    r.dim = playing ? 1 : 0.8;
  }, [playing]);

  return (
    <div ref={hostRef} className="backdrop" aria-hidden="true">
      <div className="backdrop-shade" />
    </div>
  );
}
