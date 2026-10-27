import { clampPitch, clampSpeed, effectivePitch, MAX_PITCH, MAX_SPEED, MIN_SPEED, SPEED_PRESETS, useEffects } from "@/player/effects";
import { IconMinus, IconPlus } from "./Icons";

function Stepper({
  id,
  label,
  value,
  display,
  min,
  max,
  step,
  disabled,
  onChange,
}: {
  id: string;
  label: string;
  value: number;
  display: string;
  min: number;
  max: number;
  step: number;
  disabled?: boolean;
  onChange: (v: number) => void;
}) {
  const fill = ((value - min) / (max - min)) * 100;
  return (
    <div className={`stepper${disabled ? " is-disabled" : ""}`}>
      <label htmlFor={id} className="stepper-label">{label}</label>
      <input
        id={id}
        type="range"
        className="dial"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        onDoubleClick={() => onChange(min < 0 ? 0 : 1)}
        style={{ "--fill": `${fill}%` } as React.CSSProperties}
      />
      <div className="stepper-row">
        <button type="button" className="step-btn" aria-label={`${label}: меньше`} disabled={disabled} onClick={() => onChange(value - step * 5)}>
          <IconMinus size={16} />
        </button>
        <output htmlFor={id} className="stepper-value">{display}</output>
        <button type="button" className="step-btn" aria-label={`${label}: больше`} disabled={disabled} onClick={() => onChange(value + step * 5)}>
          <IconPlus size={16} />
        </button>
      </div>
    </div>
  );
}

function Toggle({ id, title, hint, checked, onChange }: { id: string; title: string; hint: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="toggle-row" htmlFor={id}>
      <span className="toggle-text">
        <span className="toggle-title">{title}</span>
        <span className="toggle-hint">{hint}</span>
      </span>
      <input id={id} type="checkbox" className="switch" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    </label>
  );
}

export default function SoundPanel() {
  const fx = useEffects();
  const set = useEffects.setState;
  const pitch = effectivePitch(fx);
  return (
    <div className="sound-panel">
      <div className="preset-row" role="group" aria-label="Быстрая скорость">
        {SPEED_PRESETS.map((p) => (
          <button key={p} type="button" className={`preset${Math.abs(fx.speed - p) < 0.001 ? " is-on" : ""}`} onClick={() => set({ speed: p })}>
            ×{p === 1 ? "1.0" : String(p)}
          </button>
        ))}
      </div>

      <Stepper id="fx-speed" label="Скорость" value={fx.speed} display={`×${fx.speed.toFixed(2)}`} min={MIN_SPEED} max={MAX_SPEED} step={0.01} onChange={(v) => set({ speed: clampSpeed(v) })} />

      <Stepper
        id="fx-pitch"
        label={fx.linked ? "Тон (идёт за скоростью)" : "Тон"}
        value={Math.round(pitch * 10) / 10}
        display={`${pitch > 0 ? "+" : ""}${pitch.toFixed(1)} пт`}
        min={-MAX_PITCH}
        max={MAX_PITCH}
        step={0.1}
        disabled={fx.linked}
        onChange={(v) => set({ pitch: clampPitch(v) })}
      />

      <Toggle id="fx-linked" title="Тон вместе со скоростью" hint="Как пластинка: быстрее — выше, медленнее — ниже" checked={fx.linked} onChange={(linked) => set({ linked })} />
      <Toggle id="fx-keep" title="Оставлять для следующих треков" hint="Иначе всё сбросится, когда начнётся новый трек" checked={fx.keep} onChange={(keep) => set({ keep })} />
      <Toggle id="fx-reverb" title="Реверберация" hint="Объём и пространство, как в зале" checked={fx.reverb} onChange={(reverb) => set({ reverb })} />
      {fx.reverb && (
        <Stepper id="fx-reverb-amount" label="Сила реверберации" value={Math.round(fx.reverbAmount * 100)} display={`${Math.round(fx.reverbAmount * 100)}%`} min={0} max={100} step={1} onChange={(v) => set({ reverbAmount: Math.min(1, Math.max(0, v / 100)) })} />
      )}

      <button type="button" className="reset-btn" onClick={() => set({ speed: 1, pitch: 0, linked: false, reverb: false })}>
        Сбросить
      </button>
    </div>
  );
}
