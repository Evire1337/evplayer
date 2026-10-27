import { app, net } from "electron";
import { promises as fs } from "node:fs";
import path from "node:path";

// SoundCloud's public web player uses a client id that it ships inside its own scripts.
// We read it the same way the site does when it loads, and keep it for a day.

const TTL_MS = 24 * 60 * 60 * 1000;
const ID_PATTERN = /client_id\s*[:=]\s*"([a-zA-Z0-9]{32})"/;

let memory: { id: string; at: number } | null = null;

function cacheFile(): string {
  return path.join(app.getPath("userData"), "soundcloud.json");
}

async function readCache(): Promise<{ id: string; at: number } | null> {
  if (memory) return memory;
  try {
    const parsed = JSON.parse(await fs.readFile(cacheFile(), "utf8")) as { id?: string; at?: number };
    if (parsed.id && parsed.at) memory = { id: parsed.id, at: parsed.at };
  } catch {
    // no cache yet
  }
  return memory;
}

async function text(url: string): Promise<string> {
  const res = await net.fetch(url, { headers: { "user-agent": "Mozilla/5.0" } });
  if (!res.ok) throw new Error(`${url} answered ${res.status}`);
  return res.text();
}

export function extractClientId(script: string): string | null {
  return ID_PATTERN.exec(script)?.[1] ?? null;
}

export function scriptUrls(html: string): string[] {
  return [...html.matchAll(/<script[^>]+src="(https:\/\/a-v2\.sndcdn\.com\/assets\/[^"]+\.js)"/g)].map((m) => m[1]);
}

export async function getSoundCloudClientId(force = false): Promise<string> {
  const cached = await readCache();
  if (!force && cached && Date.now() - cached.at < TTL_MS) return cached.id;
  const html = await text("https://soundcloud.com/");
  // the id usually sits in one of the last bundles
  for (const url of scriptUrls(html).reverse()) {
    const id = extractClientId(await text(url).catch(() => ""));
    if (id) {
      memory = { id, at: Date.now() };
      await fs.writeFile(cacheFile(), JSON.stringify(memory)).catch(() => {});
      return id;
    }
  }
  if (cached) return cached.id;
  throw new Error("SoundCloud client id not found");
}
