import { useEffect, useState } from "react";

// The window has no system frame (so the backdrop runs to the very top); these are its
// minimize / maximize / close buttons, drawn like the Windows ones.

export default function WindowControls() {
  const api = window.evplayer?.window;
  const [maximized, setMaximized] = useState(false);
  useEffect(() => api?.onMaximized(setMaximized), [api]);
  if (!api) return null;
  return (
    <div className="win-controls">
      <button type="button" className="win-btn" aria-label="Свернуть" onClick={api.minimize}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5h10" stroke="currentColor" /></svg>
      </button>
      <button type="button" className="win-btn" aria-label={maximized ? "Восстановить" : "Развернуть"} onClick={api.toggleMaximize}>
        {maximized ? (
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M2.5 .5h7v7M.5 2.5h7v7h-7z" fill="none" stroke="currentColor" /></svg>
        ) : (
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x=".5" y=".5" width="9" height="9" fill="none" stroke="currentColor" /></svg>
        )}
      </button>
      <button type="button" className="win-btn is-close" aria-label="Закрыть" onClick={api.close}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0l10 10M10 0L0 10" stroke="currentColor" /></svg>
      </button>
    </div>
  );
}
