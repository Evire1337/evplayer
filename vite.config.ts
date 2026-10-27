import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import electron from "vite-plugin-electron/simple";

// strict content security policy for the packaged app (the dev server needs inline scripts)
const CSP = [
  "default-src 'self'",
  "script-src 'self'",
  "frame-src 'none'",
  "style-src 'self' 'unsafe-inline'",
  "font-src 'self' data:",
  "img-src 'self' data: blob: https: evplayer-media:",
  "media-src 'self' blob: https: evplayer-media:",
  "connect-src 'self' https: blob: evplayer-media:",
  "worker-src 'self' blob:",
  "object-src 'none'",
].join("; ");

const cspPlugin = {
  name: "evplayer-csp",
  apply: "build" as const,
  transformIndexHtml: () => [
    { tag: "meta", attrs: { "http-equiv": "Content-Security-Policy", content: CSP }, injectTo: "head-prepend" as const },
  ],
};

export default defineConfig({
  resolve: { alias: { "@": path.resolve(__dirname, "src") } },
  plugins: [
    react(),
    cspPlugin,
    electron({
      main: { entry: "electron/main.ts" },
      preload: {
        input: "electron/preload.ts",
        vite: { build: { rollupOptions: { output: { format: "cjs", entryFileNames: "preload.cjs" } } } },
      },
    }),
  ],
});
