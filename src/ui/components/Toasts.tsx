import { useToasts } from "../toasts";

export default function Toasts() {
  const toasts = useToasts((s) => s.toasts);
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast-pill${t.kind === "error" ? " is-error" : ""}`}>{t.text}</div>
      ))}
    </div>
  );
}
