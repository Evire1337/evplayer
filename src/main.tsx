import "./platform";
import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/onest/400.css";
import "@fontsource/onest/500.css";
import "@fontsource/onest/600.css";
import "@fontsource/onest/800.css";
import "./styles/base.css";
import "./styles/app.css";

const root = ReactDOM.createRoot(document.getElementById("root")!);

// first start after the move from the Electron version: bring its saved state over before
// any store reads the storage
void import("./app/migrate")
  .then((m) => m.migrateFromElectron())
  .then((reloading) => {
    if (reloading) return;
    return Promise.all([import("./App"), import("./app/bridge"), import("./app/shortcuts"), import("./app/visibility"), import("./app/session")]).then(
      ([{ default: App }, bridge, shortcuts, , session]) => {
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
