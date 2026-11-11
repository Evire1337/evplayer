import type { IncomingMessage, ServerResponse } from "node:http";
import { Readable } from "node:stream";
import type { Plugin } from "vite";
import { isOpenHost } from "../src/platform/net";

// What the desktop app's Rust side does for the page, done by the dev server, so the page opened
// in a plain browser (`pnpm web`, http://localhost:5173, a phone-sized window of the browser)
// can search and play like the real app:
//   /__ev/proxy/<encoded url>         requests to the services (src-tauri/src/proxy.rs does this in the app)
//   /__ev/media/remote/<encoded url>  a song's stream, with seeking (media_server.rs in the app)
//   /__ev/client-id                   the SoundCloud client id (services.rs in the app)
// Only the dev server has it: a release build never contains this file. Only the hosts the app
// itself may reach are served (isOpenHost, the same list as the app's).

export const PROXY_PATH = "/__ev/proxy/";
export const MEDIA_PATH = "/__ev/media/remote/";
export const CLIENT_ID_PATH = "/__ev/client-id";
/** the page cannot set a User-Agent: it sends it in this header (see services/youtube/direct.ts) */
const USER_AGENT_HEADER = "x-evplayer-user-agent";
const DEFAULT_USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/** request headers handed on to the service; everything else (cookies, the page's own origin) is not */
const PASSED = ["content-type", "accept", "accept-language", "range", "authorization", "x-origin", "x-goog-visitor-id", "x-goog-authuser", "x-youtube-client-name", "x-youtube-client-version"];
/** reply headers handed back to the page */
const RETURNED = ["content-type", "content-length", "content-range", "accept-ranges", "etag", "cache-control"];

/** the service address a dev path asks for, or null when it is not one the app may reach */
export function targetOf(path: string): string | null {
  for (const prefix of [PROXY_PATH, MEDIA_PATH]) {
    if (!path.startsWith(prefix)) continue;
    try {
      const url = decodeURIComponent(path.slice(prefix.length));
      return isOpenHost(url) ? url : null;
    } catch {
      return null;
    }
  }
  return null;
}

const CORS = {
  "access-control-allow-origin": "*",
  "access-control-allow-headers": "*",
  "access-control-allow-methods": "GET, POST, HEAD, OPTIONS",
  "access-control-expose-headers": "Content-Length, Content-Range, Accept-Ranges",
};

function reply(res: ServerResponse, status: number, text: string): void {
  res.writeHead(status, { ...CORS, "content-type": "text/plain; charset=utf-8" });
  res.end(text);
}

async function bodyOf(req: IncomingMessage): Promise<Buffer | undefined> {
  if (req.method === "GET" || req.method === "HEAD") return undefined;
  const parts: Buffer[] = [];
  for await (const part of req) parts.push(Buffer.from(part));
  return Buffer.concat(parts);
}

async function forward(req: IncomingMessage, res: ServerResponse, target: string): Promise<void> {
  const headers: Record<string, string> = { "user-agent": String(req.headers[USER_AGENT_HEADER] ?? DEFAULT_USER_AGENT) };
  for (const name of PASSED) {
    const value = req.headers[name];
    if (typeof value === "string") headers[name === "x-origin" ? "origin" : name] = value;
  }
  const upstream = await fetch(target, { method: req.method, headers, body: (await bodyOf(req)) as BodyInit | undefined, redirect: "follow" });
  const out: Record<string, string> = { ...CORS };
  for (const name of RETURNED) {
    const value = upstream.headers.get(name);
    if (value) out[name] = value;
  }
  // fetch has already unpacked a gzip or brotli body: its length is no longer the one the service announced
  if (upstream.headers.get("content-encoding")) delete out["content-length"];
  res.writeHead(upstream.status, out);
  if (!upstream.body || req.method === "HEAD") return void res.end();
  const stream = Readable.fromWeb(upstream.body as never);
  // the player closes a stream it has seen enough of: not an error
  res.on("close", () => stream.destroy());
  stream.on("error", () => res.end());
  stream.pipe(res);
}

// ---- the SoundCloud client id: read from the site's own scripts, like services.rs does ----

let clientId: { value: string; at: number } | null = null;
const DAY_MS = 24 * 60 * 60 * 1000;

async function pageText(url: string): Promise<string> {
  const res = await fetch(url, { headers: { "user-agent": DEFAULT_USER_AGENT } });
  if (!res.ok) throw new Error(`${url} answered ${res.status}`);
  return res.text();
}

export async function soundCloudClientId(force = false): Promise<string> {
  if (clientId && !force && Date.now() - clientId.at < DAY_MS) return clientId.value;
  const html = await pageText("https://soundcloud.com/");
  const scripts = [...html.matchAll(/<script[^>]+src="(https:\/\/a-v2\.sndcdn\.com\/assets\/[^"]+\.js)"/g)].map((m) => m[1]).reverse();
  for (const url of scripts) {
    const found = /client_id\s*[:=]\s*"([a-zA-Z0-9]{32})"/.exec(await pageText(url).catch(() => ""));
    if (found) {
      clientId = { value: found[1], at: Date.now() };
      return found[1];
    }
  }
  if (clientId) return clientId.value;
  throw new Error("SoundCloud client id not found");
}

export function webBridge(): Plugin {
  return {
    name: "evplayer-web-bridge",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        const path = req.url ?? "";
        if (!path.startsWith("/__ev/")) return next();
        try {
          if (req.method === "OPTIONS") {
            res.writeHead(204, CORS);
            return void res.end();
          }
          if (path.startsWith(CLIENT_ID_PATH)) {
            const id = await soundCloudClientId(path.includes("force=1"));
            res.writeHead(200, { ...CORS, "content-type": "text/plain" });
            return void res.end(id);
          }
          const target = targetOf(path);
          if (!target) return reply(res, 403, "This address is not one the app reaches");
          await forward(req, res, target);
        } catch (error) {
          if (!res.headersSent) reply(res, 502, error instanceof Error ? error.message : String(error));
          else res.end();
        }
      });
    },
  };
}
