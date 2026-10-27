import { useImportProgress } from "@/library/importProgress";

// a long import (a link or a Spotify export) shows its progress here
export default function ImportPill() {
  const text = useImportProgress((s) => s.text);
  const cancel = useImportProgress((s) => s.cancel);
  if (!text) return null;
  return (
    <div className="import-pill glass" role="status">
      <span className="spinner" aria-hidden="true" />
      <span>{text}</span>
      {cancel && (
        <button type="button" className="toast-secondary" onClick={cancel}>
          Отменить
        </button>
      )}
    </div>
  );
}
