import { useEffect, useState } from "react";
import { cancelKeeping, watchJob, type KeepJob } from "./keep";

// the panel for songs being saved for offline: how many are done, and a cancel button
export default function KeepPill() {
  const [job, setJob] = useState<KeepJob | null>(null);
  useEffect(() => watchJob(setJob), []);
  if (!job) return null;
  const share = job.total ? Math.min(1, job.done / job.total) : 0;
  return (
    <div className="import-panel keep-panel glass" role="status" aria-live="polite">
      <div className="import-head">
        <span className="spinner" aria-hidden="true" />
        <span className="import-title">Скачиваю для офлайн</span>
        <button type="button" className="toast-secondary" onClick={cancelKeeping}>Отменить</button>
      </div>
      <div className="import-bar"><span style={{ width: `${share * 100}%` }} /></div>
      <div className="import-meta">
        <span>{job.done} из {job.total} · {Math.round(share * 100)}%</span>
        {job.failed > 0 && <span className="import-state">не получилось: {job.failed}</span>}
      </div>
    </div>
  );
}
