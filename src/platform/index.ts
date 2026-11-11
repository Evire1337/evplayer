import { isPhone } from "./device";
import { installTauri } from "./tauri";
import { installWeb } from "./web";

// Runs before anything else in the page: the rest of the app reads `window.evplayer` while
// its modules load. In a plain browser (tests, the dev server opened directly) there is no
// desktop side and the app works without those features.

if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) installTauri();
// `pnpm web`: the dev server stands in for the desktop side of the services (tools/webBridge.ts)
else if (import.meta.env.DEV && typeof window !== "undefined") installWeb();

// the phone layout is switched on before the first paint
if (isPhone) document.documentElement.classList.add("is-phone");
