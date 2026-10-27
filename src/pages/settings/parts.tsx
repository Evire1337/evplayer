import type { ReactNode } from "react";

// building blocks of the settings page

export function TabHeader({ icon, title, description, hue }: { icon: ReactNode; title: string; description: string; hue: number }) {
  return (
    <header className="set-head">
      <span className="set-head-icon" style={{ "--hue": hue } as React.CSSProperties}>{icon}</span>
      <span className="set-head-text">
        <h2 className="set-head-title">{title}</h2>
        <span className="set-head-desc">{description}</span>
      </span>
    </header>
  );
}

export function Card({ title, hint, children, actions }: { title?: string; hint?: string; children: ReactNode; actions?: ReactNode }) {
  return (
    <section className="set-card">
      {(title || actions) && (
        <div className="set-card-head">
          <span>
            {title && <h3 className="set-card-title">{title}</h3>}
            {hint && <p className="set-card-hint">{hint}</p>}
          </span>
          {actions && <span className="set-card-actions">{actions}</span>}
        </div>
      )}
      <div className="set-card-body">{children}</div>
    </section>
  );
}

export function Row({ title, hint, children, icon }: { title: string; hint?: ReactNode; children?: ReactNode; icon?: ReactNode }) {
  return (
    <div className="set-row">
      {icon && <span className="set-row-icon">{icon}</span>}
      <span className="set-row-text">
        <span className="set-row-title">{title}</span>
        {hint && <span className="set-row-hint">{hint}</span>}
      </span>
      {children && <span className="set-row-control">{children}</span>}
    </div>
  );
}

export function Switch({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return <input type="checkbox" role="switch" className="switch" aria-label={label} checked={checked} onChange={(e) => onChange(e.target.checked)} />;
}

/** a row whose whole width toggles the switch */
export function SwitchRow({ title, hint, checked, onChange, icon, disabled }: { title: string; hint?: string; checked: boolean; onChange: (v: boolean) => void; icon?: ReactNode; disabled?: boolean }) {
  return (
    <label className={`set-row is-clickable${disabled ? " is-disabled" : ""}`}>
      {icon && <span className="set-row-icon">{icon}</span>}
      <span className="set-row-text">
        <span className="set-row-title">{title}</span>
        {hint && <span className="set-row-hint">{hint}</span>}
      </span>
      <span className="set-row-control">
        <input type="checkbox" role="switch" className="switch" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      </span>
    </label>
  );
}

export function Pill({ tone, children }: { tone: "ok" | "warn" | "off" | "info"; children: ReactNode }) {
  return <span className={`pill is-${tone}`}>{children}</span>;
}
