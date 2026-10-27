import { useEffect, useRef } from "react";
import { BackdropRenderer } from "./BackdropRenderer";

export default function Backdrop({ image, playing }: { image?: string; playing: boolean }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rendererRef = useRef<BackdropRenderer | null>(null);

  useEffect(() => {
    if (!canvasRef.current) return;
    try {
      rendererRef.current = new BackdropRenderer(canvasRef.current);
      rendererRef.current.start();
    } catch {
      rendererRef.current = null; // no WebGL: the CSS gradient behind stays visible
    }
    return () => {
      rendererRef.current?.destroy();
      rendererRef.current = null;
    };
  }, []);

  useEffect(() => {
    rendererRef.current?.setImage(image ?? null);
  }, [image]);

  useEffect(() => {
    const r = rendererRef.current;
    if (!r) return;
    // slower and dimmer while paused, so the screen calms down
    r.speed = playing ? 1 : 0.25;
    r.dim = playing ? 1 : 0.8;
  }, [playing]);

  return (
    <div className="backdrop" aria-hidden="true">
      <canvas ref={canvasRef} className="backdrop-canvas" />
      <div className="backdrop-shade" />
    </div>
  );
}
