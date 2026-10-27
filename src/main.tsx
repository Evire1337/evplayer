import "./platform";
import { isPhone } from "./platform/device";
import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/onest/400.css";
import "@fontsource/onest/500.css";
import "@fontsource/onest/600.css";
import "@fontsource/onest/800.css";
import "./styles/base.css";
import "./styles/app.css";
import "./styles/phone.css";

const root = ReactDOM.createRoot(document.getElementById("root")!);

// first start after the move from the Electron version: bring its saved state over before
// any store reads the storage
void import("./app/migrate")
  .then((m) => m.migrateFromElectron())
  .then((reloading) => {
    if (reloading) return;
    return Promise.all([import("./App"), import("./app/bridge"), import("./app/shortcuts"), import("./app/visibility"), import("./app/session"), import("./settings/store")]).then(
      ([{ default: App }, bridge, shortcuts, , session, { useSettings }]) => {
        // phones keep no cache: only the library, playlists, settings and statistics stay
        if (isPhone) useSettings.getState().set({ noCache: true, noServiceCache: true, noLoudnessMemory: true });
        session.restoreSession();
        bridge.startBridge();
        shortcuts.startShortcuts();
        root.render(
          <React.StrictMode>
            <App />
          </React.StrictMode>,
        );
      },
    );
  });
