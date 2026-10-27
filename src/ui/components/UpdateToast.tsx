import { useEffect, useState } from "react";
import type { UpdateState } from "@/platform/api";

// "A new version is ready" with a restart button; nothing shows until there is one
export default function UpdateToast() {
  const [state, setState] = useState<UpdateState>({ status: "idle" });
  const [dismissed, setDismissed] = useState(false);
  useEffect(() => {
    const bridge = window.evplayer?.updates;
    if (!bridge) return;
    void bridge.get().then(setState);
    return bridge.onState(setState);
  }, []);
  if (state.status !== "ready" || dismissed) return null;
  return (
    <div className="toast glass" role="status">
      <span>
        Вышла версия <b>{state.version}</b>. Она установится при перезапуске.
      </span>
      <button type="button" className="toast-primary" onClick={() => void window.evplayer?.updates.install()}>
        Перезапустить
      </button>
      <button type="button" className="toast-secondary" onClick={() => setDismissed(true)}>
        Позже
      </button>
    </div>
  );
}
