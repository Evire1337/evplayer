import { acceleratorFromEvent, appAction } from "@/settings/shortcuts";
import { useSettings } from "@/settings/store";
import { runAction } from "./actions";

// keys inside the window; they can be changed in Settings → Keys

function typing(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
}

/** while a key is being recorded in the settings, shortcuts stay quiet */
export const recording = { active: false };

export function startShortcuts(): void {
  window.addEventListener("keydown", (e) => {
    if (e.defaultPrevented || recording.active || e.repeat && !/Arrow/.test(e.code)) return;
    const acc = acceleratorFromEvent(e);
    if (!acc) return;
    const hasMod = e.ctrlKey || e.metaKey || e.altKey;
    // plain keys never fire while you type or while a slider/list has focus
    if (typing(e.target) && !hasMod) return;
    const focused = document.activeElement;
    const onControl = focused && focused !== document.body && (focused.tagName === "INPUT" || focused.getAttribute("role") === "slider");
    if (!hasMod && onControl && /^(Left|Right|Up|Down)$/.test(acc)) return;
    if (acc === "Space" && focused?.tagName === "BUTTON") return;
    const action = appAction(useSettings.getState().shortcuts, acc);
    if (!action) return;
    e.preventDefault();
    runAction(action);
  });
}
