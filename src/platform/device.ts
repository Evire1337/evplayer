// Which kind of device the app runs on. Phones get their own layout, no desktop-only
// settings (tray, keys, mini player window, Discord), and keep no cache on the phone.

export const isPhone: boolean = typeof navigator !== "undefined" && /Android|iPhone|iPad/i.test(navigator.userAgent);

/** settings tabs that only make sense on a computer */
export const DESKTOP_ONLY_TABS = new Set(["keys", "overlay", "discord"]);
