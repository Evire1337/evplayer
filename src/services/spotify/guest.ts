import type { NamedTrack } from "@/library/matchTracks";

// Whole Spotify playlists without signing in. Spotify's own web player reads a playlist
// with a short-lived "guest" token; the same token works here. The token needs a one-time
// code (TOTP) whose secret is shipped inside the web player's script, so the script is read
// first. All requests go through the app's proxy (src/platform/net.ts, src-tauri/src/proxy.rs).
// This is not an official API: if Spotify changes the player, callers fall back to the embed
// page (100 songs) or to the official Web API (webApi.ts).

const PLAYER_VERSION = "1.3.2.216.g7f3fbea1";
const CLIENT_ID = "d8a5ed958d274c2e8ee717e6a4b0971d";
const PATHFINDER_HASH = "86dde7b9d9356e2369414647cf6950cfed96e778e129cfdfc99aea6c1613b3b0";
const PAGE_LIMIT = 100;
const MAX_PAGES = 60;
const PAGE_ATTEMPTS = 3;

type Json = Record<string, unknown>;

const rec = (v: unknown): Json | undefined => (v && typeof v === "object" && !Array.isArray(v) ? (v as Json) : undefined);
const at = (v: unknown, ...path: string[]): Json | undefined => path.reduce<Json | undefined>((o, k) => rec(o?.[k]), rec(v));
const str = (v: unknown): string | undefined => (typeof v === "string" && v ? v : undefined);
const num = (v: unknown): number | undefined => (typeof v === "number" && Number.isFinite(v) ? v : undefined);

const sleep = (ms: number, signal?: AbortSignal) =>
  new Promise<void>((resolve, reject) => {
    if (signal?.aborted) return reject(new DOMException("aborted", "AbortError"));
    const t = setTimeout(resolve, ms);
    signal?.addEventListener("abort", () => (clearTimeout(t), reject(new DOMException("aborted", "AbortError"))), { once: true });
  });

async function text(url: string, signal?: AbortSignal): Promise<string | undefined> {
  try {
    const res = await fetch(url, { signal });
    return res.ok ? await res.text() : undefined;
  } catch (e) {
    if (signal?.aborted) throw e;
    return undefined;
  }
}

// ---- the one-time code ----------------------------------------------------------------

export interface TotpSecret {
  secret: string;
  version: number;
}

const PLAYER_SCRIPT = /src="(https:\/\/open\.spotifycdn\.com\/cdn\/build\/(?:mobile-)?web-player\/(?:mobile-)?web-player\.[a-f0-9]+\.js)"/;
const SECRET_ENTRY = /\{secret:(['"])((?:\\.|(?!\1)[^\\])*)\1,version:(\d+)\}/g;

/** the newest secret listed in the player's script */
export function extractSecret(js: string): TotpSecret | undefined {
  let newest: TotpSecret | undefined;
  for (const m of js.matchAll(SECRET_ENTRY)) {
    const candidate = { secret: m[2].replace(/\\(.)/g, "$1"), version: Number(m[3]) };
    if (!newest || candidate.version > newest.version) newest = candidate;
  }
  return newest;
}

async function fetchSecret(signal?: AbortSignal): Promise<TotpSecret> {
  const html = await text("https://open.spotify.com/", signal);
  const script = html?.match(PLAYER_SCRIPT)?.[1];
  const js = script ? await text(script, signal) : undefined;
  const secret = js ? extractSecret(js) : undefined;
  if (!secret) throw new Error("Spotify сейчас недоступен, попробуйте позже");
  return secret;
}

async function serverTime(signal?: AbortSignal): Promise<number | null> {
  const body = await text("https://open.spotify.com/api/server-time", signal);
  try {
    const n = Number(rec(body ? JSON.parse(body) : null)?.serverTime);
    return Number.isFinite(n) ? n : null;
  } catch {
    return null;
  }
}

export function secretBytes(secret: string): Uint8Array<ArrayBuffer> {
  const mixed = secret.split("").map((c, i) => c.charCodeAt(0) ^ ((i % 33) + 9));
  const encoded = new TextEncoder().encode(mixed.join(""));
  const bytes = new Uint8Array(new ArrayBuffer(encoded.length));
  bytes.set(encoded);
  return bytes;
}

export async function totp(secret: Uint8Array<ArrayBuffer>, timeMs: number): Promise<string> {
  const key = await crypto.subtle.importKey("raw", secret, { name: "HMAC", hash: "SHA-1" }, false, ["sign"]);
  const counter = new ArrayBuffer(8);
  new DataView(counter).setBigInt64(0, BigInt(Math.floor(timeMs / 1000 / 30)));
  const hmac = new Uint8Array(await crypto.subtle.sign("HMAC", key, counter));
  const o = hmac[hmac.length - 1] & 0xf;
  const code = ((hmac[o] & 0x7f) << 24) | ((hmac[o + 1] & 0xff) << 16) | ((hmac[o + 2] & 0xff) << 8) | (hmac[o + 3] & 0xff);
  return (code % 1_000_000).toString().padStart(6, "0");
}

// ---- guest session ---------------------------------------------------------------------

interface Session {
  accessToken: string;
  clientToken: string;
  expiresAt: number;
}

let cached: Session | null = null;

async function accessToken(signal?: AbortSignal): Promise<{ accessToken: string; expiresAt: number }> {
  const { secret, version } = await fetchSecret(signal);
  const bytes = secretBytes(secret);
  const server = await serverTime(signal);
  const params = new URLSearchParams({
    reason: "init",
    productType: "web-player",
    totp: await totp(bytes, Date.now()),
    totpServer: server ? await totp(bytes, server * 1000) : "unavailable",
    totpVer: String(version),
  });
  const res = await fetch(`https://open.spotify.com/api/token?${params}`, { headers: { accept: "application/json" }, signal });
  if (!res.ok) throw new Error(`Spotify не выдал гостевой доступ (${res.status})`);
  const body = rec(await res.json());
  const token = str(body?.accessToken);
  if (!token) throw new Error("Spotify не выдал гостевой доступ");
  return { accessToken: token, expiresAt: num(body?.accessTokenExpirationTimestampMs) ?? Date.now() + 3_500_000 };
}

async function clientToken(signal?: AbortSignal): Promise<string> {
  const res = await fetch("https://clienttoken.spotify.com/v1/clienttoken", {
    method: "POST",
    headers: { accept: "application/json", "content-type": "application/json" },
    body: JSON.stringify({
      client_data: {
        client_version: PLAYER_VERSION,
        client_id: CLIENT_ID,
        js_sdk_data: {
          device_brand: "unknown",
          device_model: "unknown",
          os: "windows",
          os_version: "unknown",
          device_id: crypto.randomUUID(),
          device_type: "computer",
          platform_identifier: "web-player",
        },
      },
    }),
    signal,
  });
  return str(at(await res.json().catch(() => null), "granted_token")?.token) ?? "";
}

async function session(signal?: AbortSignal): Promise<Session> {
  if (cached && Date.now() < cached.expiresAt - 60_000) return cached;
  const { accessToken: token, expiresAt } = await accessToken(signal);
  cached = { accessToken: token, clientToken: await clientToken(signal), expiresAt };
  return cached;
}

// ---- playlist pages --------------------------------------------------------------------

function pageUrl(playlistId: string, offset: number): string {
  const params = new URLSearchParams({
    operationName: "fetchPlaylistContents",
    variables: JSON.stringify({ uri: `spotify:playlist:${playlistId}`, offset, limit: PAGE_LIMIT }),
    extensions: JSON.stringify({ persistedQuery: { version: 1, sha256Hash: PATHFINDER_HASH } }),
  });
  return `https://api-partner.spotify.com/pathfinder/v1/query?${params}`;
}

/** one playlist row of the pathfinder reply as a song to look up */
export function mapItem(item: unknown): NamedTrack | undefined {
  const track = at(item, "itemV2", "data");
  if (!track || track.__typename !== "Track") return undefined;
  const title = str(track.name)?.trim();
  if (!title) return undefined;
  const artists = (Array.isArray(at(track, "artists")?.items) ? (at(track, "artists")!.items as unknown[]) : [])
    .map((a) => str(at(a, "profile")?.name) ?? str(rec(a)?.name))
    .filter((n): n is string => Boolean(n));
  return { title, artists, durationMs: num(at(track, "trackDuration")?.totalMilliseconds) };
}

async function page(playlistId: string, offset: number, signal?: AbortSignal): Promise<Json | undefined> {
  let s = await session(signal);
  for (let attempt = 1; attempt <= PAGE_ATTEMPTS; attempt++) {
    try {
      const res = await fetch(pageUrl(playlistId, offset), {
        headers: {
          authorization: `Bearer ${s.accessToken}`,
          ...(s.clientToken ? { "client-token": s.clientToken } : {}),
          accept: "application/json",
          "app-platform": "WebPlayer",
          "spotify-app-version": PLAYER_VERSION,
        },
        signal,
      });
      if (res.status === 429) {
        await sleep(Math.min(30, Number(res.headers.get("retry-after") || 3)) * 1000, signal);
        continue;
      }
      if (res.status === 401 || res.status === 403) {
        cached = null;
        s = await session(signal);
        continue;
      }
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const content = at(await res.json(), "data", "playlistV2", "content");
      if (!content) throw new Error("в ответе нет списка треков");
      return content;
    } catch (e) {
      if (signal?.aborted) throw e;
      if (attempt < PAGE_ATTEMPTS) await sleep(500 * attempt, signal);
    }
  }
  return undefined;
}

/** every song of a public playlist, any length, without signing in */
export async function spotifyGuestPlaylist(
  playlistId: string,
  signal?: AbortSignal,
  onPage?: (count: number, total: number) => void,
): Promise<{ tracks: NamedTrack[]; total?: number }> {
  const tracks: NamedTrack[] = [];
  let total: number | undefined;
  for (let offset = 0, n = 0; n < MAX_PAGES && (total === undefined || offset < total); n++, offset += PAGE_LIMIT) {
    const content = await page(playlistId, offset, signal);
    if (!content) {
      if (n === 0) throw new Error("Spotify не отдал плейлист без входа");
      break;
    }
    total = num(content.totalCount) ?? total;
    const items = Array.isArray(content.items) ? content.items : [];
    if (!items.length) break;
    for (const item of items) {
      const t = mapItem(item);
      if (t) tracks.push(t);
    }
    onPage?.(tracks.length, total ?? tracks.length);
    await sleep(50, signal);
  }
  return { tracks, total };
}

export function resetGuestSession(): void {
  cached = null;
}
