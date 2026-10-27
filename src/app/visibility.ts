import { create } from "zustand";

// Is the window on screen? False when minimized, hidden in the tray or covered by a
// fullscreen app. Animations (backdrop, lyrics, spectrum) stop while it is false, so a
// player playing in the background costs almost no CPU or GPU.

export const useVisible = create<{ visible: boolean }>(() => ({ visible: typeof document === "undefined" ? true : !document.hidden }));

if (typeof document !== "undefined") {
  let fromMain = true;
  const update = () => {
    const visible = fromMain && !document.hidden;
    if (useVisible.getState().visible !== visible) useVisible.setState({ visible });
    document.documentElement.toggleAttribute("data-hidden", !visible);
  };
  document.addEventListener("visibilitychange", update);
  window.evplayer?.onWindowVisible((v) => {
    fromMain = v;
    update();
  });
}
