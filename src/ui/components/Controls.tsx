import { IconMinus, IconPlus } from "./Icons";

// small form controls shared by the sound panel and the settings page

export function Stepper({
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

export function Toggle({ id, title, hint, checked, onChange }: { id: string; title: string; hint?: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="toggle-row" htmlFor={id}>
      <span className="toggle-text">
        <span className="toggle-title">{title}</span>
        {hint && <span className="toggle-hint">{hint}</span>}
      </span>
      <input id={id} type="checkbox" className="switch" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    </label>
  );
}


export function Segmented<T extends string>({ value, options, onChange, label }: { value: T; options: Array<{ value: T; label: string }>; onChange: (v: T) => void; label: string }) {
  return (
    <div className="segmented" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button key={o.value} type="button" role="radio" aria-checked={o.value === value} className={o.value === value ? "is-on" : ""} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function SettingRow({ title, hint, children }: { title: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="setting-row">
      <span className="toggle-text">
        <span className="toggle-title">{title}</span>
        {hint && <span className="toggle-hint">{hint}</span>}
      </span>
      <div className="setting-control">{children}</div>
    </div>
  );
}
