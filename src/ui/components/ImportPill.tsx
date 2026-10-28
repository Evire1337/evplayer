import { useEffect, useState } from "react";
import { useImportProgress } from "@/library/importProgress";

/** after this long without a change the panel says the service is slow */
const STALL_MS = 12_000;

function seconds(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  return s < 60 ? `${s} с` : `${Math.floor(s / 60)} мин ${s % 60} с`;
}

// a long import (a link, a code, a Spotify or Yandex export): how far it got and whether it
// is still going
export default function ImportPill() {
  const { text, cancel, done, total, found, startedAt, changedAt, result } = useImportProgress();
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!text) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [text]);

  if (!text && result) {
    return (
      <div className={`import-panel glass ${result.ok ? "is-ok" : "is-error"}`} role="status">
        <div className="import-head">
          <span className="import-dot" aria-hidden="true" />
          <span className="import-title">{result.text}</span>
        </div>
      </div>
    );
  }
  if (!text) return null;

  const stalled = now - changedAt > STALL_MS;
  const share = done !== null && total ? Math.min(1, done / total) : null;
  return (
    <div className={`import-panel glass${stalled ? " is-stalled" : ""}`} role="status" aria-live="polite">
      <div className="import-head">
        <span className="spinner" aria-hidden="true" />
        <span className="import-title">{text}</span>
        {cancel && (
          <button type="button" className="toast-secondary" onClick={cancel}>
            Отменить
          </button>
        )}
      </div>
      <div className={`import-bar${share === null ? " is-unknown" : ""}`}>
        <span style={share === null ? undefined : { width: `${share * 100}%` }} />
      </div>
      <div className="import-meta">
        <span>
          {done !== null && total !== null ? `${done} из ${total}${total ? ` · ${Math.round((share ?? 0) * 100)}%` : ""}` : "Готовлю список…"}
          {found !== null ? ` · найдено ${found}` : ""}
        </span>
        <span className="import-state">{stalled ? `Сервис отвечает медленно · нет изменений ${seconds(now - changedAt)}` : `Идёт загрузка · ${seconds(now - startedAt)}`}</span>
      </div>
    </div>
  );
}
