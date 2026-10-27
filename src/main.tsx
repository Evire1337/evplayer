import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/onest/400.css";
import "@fontsource/onest/500.css";
import "@fontsource/onest/600.css";
import "@fontsource/onest/800.css";
import "./styles/app.css";

const root = ReactDOM.createRoot(document.getElementById("root")!);

// the same page also serves the mini player window (index.html#overlay); that one only
// shows state and must not start a second player
if (location.hash === "#overlay") {
  document.documentElement.classList.add("is-overlay");
  void import("./overlay/OverlayApp").then(({ default: OverlayApp }) => root.render(<OverlayApp />));
} else {
  void Promise.all([import("./App"), import("./app/bridge"), import("./app/shortcuts"), import("./app/visibility"), import("./app/session")]).then(
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
}
