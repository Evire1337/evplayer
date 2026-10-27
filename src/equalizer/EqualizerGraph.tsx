import { useSettings } from "@/settings/store";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  BAND_COLORS,
  bandResponseDb,
  clamp,
  freqToX,
  MAX_GAIN,
  MAX_Q,
  MIN_Q,
  totalResponseDb,
  xToFreq,
  type EqBand,
} from "./eqModel";
import { getEqBus } from "./eqAudio";

const HEIGHT = 260;
const PAD_Y = 14;
const RANGE_DB = 27; // the view shows a bit more than the +-24 dB the bands can reach
const GRID_FREQS = [30, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
const GRID_DB = [-24, -12, 0, 12, 24];
const POINTS = 220;

const dbToY = (db: number) => PAD_Y + ((RANGE_DB - db) / (2 * RANGE_DB)) * (HEIGHT - 2 * PAD_Y);
const yToDb = (y: number) => RANGE_DB - ((y - PAD_Y) / (HEIGHT - 2 * PAD_Y)) * 2 * RANGE_DB;

interface Props {
  bands: EqBand[];
  enabled: boolean;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onChange: (id: string, patch: Partial<EqBand>) => void;
}

export default function EqualizerGraph({ bands, enabled, selectedId, onSelect, onChange }: Props) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [width, setWidth] = useState(640);
  const dragRef = useRef<{ id: string; pointerId: number } | null>(null);

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => setWidth(Math.max(320, Math.round(entry.contentRect.width))));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // live spectrum of what is playing, drawn behind the curves
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = HEIGHT * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    let raf = 0;
    let data: Uint8Array<ArrayBuffer> | null = null;
    // memory saver: no live spectrum behind the curve
    if (useSettings.getState().memorySaver) return;
    const draw = () => {
      ctx.clearRect(0, 0, width, HEIGHT);
      const bus = getEqBus();
      if (bus && enabled) {
        const analyser = bus.analyser;
        if (!data || data.length !== analyser.frequencyBinCount) data = new Uint8Array(analyser.frequencyBinCount);
        analyser.getByteFrequencyData(data);
        const nyquist = bus.context.sampleRate / 2;
        ctx.beginPath();
        ctx.moveTo(0, HEIGHT);
        for (let x = 0; x <= width; x += 2) {
          const f = xToFreq(x, width);
          const bin = Math.min(data.length - 1, Math.round((f / nyquist) * data.length));
          const v = data[bin] / 255;
          ctx.lineTo(x, HEIGHT - v * (HEIGHT - PAD_Y) * 0.9);
        }
        ctx.lineTo(width, HEIGHT);
        ctx.closePath();
        const gradient = ctx.createLinearGradient(0, 0, 0, HEIGHT);
        gradient.addColorStop(0, "rgba(255,255,255,0.20)");
        gradient.addColorStop(1, "rgba(255,255,255,0.02)");
        ctx.fillStyle = gradient;
        ctx.fill();
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [width, enabled]);

  const freqs = useMemo(() => Array.from({ length: POINTS + 1 }, (_, i) => xToFreq((i / POINTS) * width, width)), [width]);

  const bandPaths = useMemo(
    () =>
      bands.map((band) => {
        if (!band.enabled || band.gain === 0) return "";
        const zero = dbToY(0);
        let d = `M 0 ${zero}`;
        freqs.forEach((f, i) => {
          d += ` L ${(i / POINTS) * width} ${dbToY(bandResponseDb(band, f))}`;
        });
        return `${d} L ${width} ${zero} Z`;
      }),
    [bands, freqs, width],
  );

  const totalPath = useMemo(() => {
    let d = "";
    freqs.forEach((f, i) => {
      d += `${i === 0 ? "M" : " L"} ${(i / POINTS) * width} ${dbToY(totalResponseDb(bands, f))}`;
    });
    return d;
  }, [bands, freqs, width]);

  const pointerToBand = useCallback(
    (event: React.PointerEvent | PointerEvent) => {
      const rect = wrapRef.current!.getBoundingClientRect();
      const x = clamp(event.clientX - rect.left, 0, width);
      const y = clamp(event.clientY - rect.top, 0, HEIGHT);
      return { frequency: xToFreq(x, width), gain: clamp(yToDb(y), -MAX_GAIN, MAX_GAIN) };
    },
    [width],
  );

  const onPointerDown = (band: EqBand) => (event: React.PointerEvent<SVGGElement>) => {
    event.preventDefault();
    onSelect(band.id);
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    dragRef.current = { id: band.id, pointerId: event.pointerId };
  };

  const onPointerMove = (event: React.PointerEvent<SVGGElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    const next = pointerToBand(event);
    // shift locks the frequency so only the level moves
    onChange(drag.id, event.shiftKey ? { gain: next.gain } : next);
  };

  const onPointerUp = (event: React.PointerEvent<SVGGElement>) => {
    if (dragRef.current?.pointerId === event.pointerId) dragRef.current = null;
  };

  const onWheel = (band: EqBand) => (event: React.WheelEvent) => {
    if (band.type !== "peaking") return;
    event.preventDefault();
    const factor = event.deltaY < 0 ? 1.12 : 1 / 1.12;
    onChange(band.id, { q: clamp(band.q * factor, MIN_Q, MAX_Q) });
  };

  return (
    <div
      ref={wrapRef}
      className={`eq-graph${enabled ? "" : " is-off"}`}
      style={{ height: HEIGHT, touchAction: "none" }}
    >
      <canvas ref={canvasRef} className="eq-graph-canvas" style={{ width, height: HEIGHT }} />
      <svg width={width} height={HEIGHT} className="eq-graph-svg">
        {GRID_FREQS.map((f) => (
          <g key={f}>
            <line x1={freqToX(f, width)} x2={freqToX(f, width)} y1={0} y2={HEIGHT} stroke="rgba(255,255,255,0.06)" />
            <text x={freqToX(f, width) + 3} y={HEIGHT - 4} fontSize={9} fill="rgba(255,255,255,0.35)">
              {f >= 1000 ? `${f / 1000}k` : f}
            </text>
          </g>
        ))}
        {GRID_DB.map((db) => (
          <g key={db}>
            <line x1={0} x2={width} y1={dbToY(db)} y2={dbToY(db)} stroke={db === 0 ? "rgba(255,255,255,0.16)" : "rgba(255,255,255,0.06)"} />
            <text x={width - 4} y={dbToY(db) - 3} fontSize={9} textAnchor="end" fill="rgba(255,255,255,0.35)">
              {db > 0 ? `+${db}` : db}
            </text>
          </g>
        ))}

        {bandPaths.map((d, i) =>
          d ? (
            <path
              key={bands[i].id}
              d={d}
              fill={BAND_COLORS[i % BAND_COLORS.length]}
              fillOpacity={bands[i].id === selectedId ? 0.32 : 0.16}
              stroke={BAND_COLORS[i % BAND_COLORS.length]}
              strokeOpacity={0.55}
              strokeWidth={1}
            />
          ) : null,
        )}

        <path d={totalPath} fill="none" stroke="#ffffff" strokeWidth={2} strokeLinejoin="round" />

        {bands.map((band, i) => {
          const cx = freqToX(band.frequency, width);
          const cy = dbToY(band.enabled ? band.gain : 0);
          const color = BAND_COLORS[i % BAND_COLORS.length];
          const selected = band.id === selectedId;
          return (
            <g
              key={band.id}
              onPointerDown={onPointerDown(band)}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
              onPointerCancel={onPointerUp}
              onWheel={onWheel(band)}
              onDoubleClick={() => onChange(band.id, { gain: 0 })}
              style={{ cursor: "grab" }}
            >
              <circle cx={cx} cy={cy} r={16} fill="transparent" />
              {selected && <circle cx={cx} cy={cy} r={11} fill="none" stroke={color} strokeOpacity={0.45} strokeWidth={2} />}
              <circle
                cx={cx}
                cy={cy}
                r={7}
                fill={band.enabled ? color : "#555"}
                stroke="#0b0b0c"
                strokeWidth={2}
              />
              <text x={cx} y={cy + 3} fontSize={8} fontWeight={700} textAnchor="middle" fill="#0b0b0c" pointerEvents="none">
                {i + 1}
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}
