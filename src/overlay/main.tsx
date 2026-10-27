import "../platform";
import ReactDOM from "react-dom/client";
import "@fontsource/onest/500.css";
import "@fontsource/onest/700.css";
import "../styles/base.css";
import "./overlay.css";
import OverlayApp from "./OverlayApp";

// The mini player is its own small page: none of the main window's code, styles or player
// is loaded here, so the window stays light.

ReactDOM.createRoot(document.getElementById("root")!).render(<OverlayApp />);
