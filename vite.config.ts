import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { webBridge } from "./tools/webBridge";

export default defineConfig({
  resolve: { alias: { "@": path.resolve(__dirname, "src") } },
  // webBridge: only in the dev server, so a page in a plain browser can reach the services
  plugins: [react(), webBridge()],
  // the content security policy is set by Tauri (src-tauri/tauri.conf.json)
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: {
    target: "es2022",
    rollupOptions: { input: { main: path.resolve(__dirname, "index.html"), overlay: path.resolve(__dirname, "overlay.html") } },
  },
});
