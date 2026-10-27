import { installTauri } from "./tauri";

// Runs before anything else in the page: the rest of the app reads `window.evplayer` while
// its modules load. In a plain browser (tests, the dev server opened directly) there is no
// desktop side and the app works without those features.

if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) installTauri();
