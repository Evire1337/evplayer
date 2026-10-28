import { mediaStream } from "@/platform/net";

// YouTube songs played straight from YouTube (not from a SoundCloud copy).
// YouTube hands out audio addresses to its own apps; asking as those apps (a VR headset
// app and a visionOS app) gives plain addresses that need no extra decoding. The page
// cannot set a User-Agent, so it sends it in USER_AGENT_HEADER and the app's proxy
// (src-tauri/src/proxy.rs) swaps it in. The sound itself is fetched by the app's media
// server (media_server.rs), which asks googlevideo in small pieces like the apps do.

export const USER_AGENT_HEADER = "x-evplayer-user-agent";

export interface Profile {
  clientName: string;
  clientVersion: string;
  clientId: string;
  userAgent: string;
  extra?: Record<string, string | number>;
}

export const VISIONOS: Profile = {
  clientName: "VISIONOS",
  clientVersion: "1.02",
  clientId: "101",
  userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
  extra: { osName: "visionOS", osVersion: "26.5.23O471", deviceMake: "Apple", deviceModel: "RealityDevice17,1" },
};

export const ANDROID_VR: Profile = {
  clientName: "ANDROID_VR",
  clientVersion: "1.65.10",
  clientId: "28",
  userAgent: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
  extra: { osName: "Android", osVersion: "12L", deviceMake: "Oculus", deviceModel: "Quest 3", androidSdkVersion: 32 },
};

export const PROFILES: readonly Profile[] = [VISIONOS, ANDROID_VR];

export interface DirectStream {
  url: string;
  mimeType: string;
  codec: "opus" | "aac";
  bitrate: number;
  itag: number;
  contentLength?: number;
  /** ms since epoch; the address stops working after that */
  expiresAt: number | null;
}

type Json = Record<string, unknown>;
const rec = (v: unknown): Json | undefined => (v && typeof v === "object" && !Array.isArray(v) ? (v as Json) : undefined);

// ---- visitor id (YouTube wants one on every request) -------------------------------------

const VISITOR = /Cg[s-v][A-Za-z0-9_-]{20,}(?:%3D|=){0,2}/;

export function parseVisitorData(swJsData: string): string | null {
  try {
    const body = swJsData.startsWith(")]}'") ? swJsData.slice(swJsData.indexOf("\n") + 1) : swJsData;
    const arr = (v: unknown): unknown[] => (Array.isArray(v) ? v : []);
    const value = arr(arr(arr(arr(arr(JSON.parse(body))[0])[2])[0])[0])[13];
    if (typeof value === "string" && VISITOR.test(value)) return value;
  } catch {
    // not JSON: fall through to the pattern
  }
  return swJsData.match(VISITOR)?.[0] ?? null;
}

let visitor: { value: string; until: number } | null = null;
let visitorPending: Promise<string> | null = null;

function visitorData(): Promise<string> {
  if (visitor && visitor.until > Date.now()) return Promise.resolve(visitor.value);
  visitorPending ??= (async () => {
    try {
      const res = await fetch("https://www.youtube.com/sw.js_data");
      const value = res.ok ? parseVisitorData(await res.text()) : null;
      if (value) visitor = { value, until: Date.now() + 5.5 * 3600_000 };
      return value ?? visitor?.value ?? "";
    } catch {
      return visitor?.value ?? "";
    } finally {
      visitorPending = null;
    }
  })();
  return visitorPending;
}

// ---- the player reply -------------------------------------------------------------------

interface Format {
  itag: number;
  url: string;
  mimeType: string;
  bitrate: number;
  contentLength?: number;
}

const BEST_ITAGS = [251, 140, 250, 249, 139];

export function audioFormats(response: unknown): Format[] {
  const list = rec(rec(response)?.streamingData)?.adaptiveFormats;
  const out: Format[] = [];
  for (const raw of Array.isArray(list) ? list : []) {
    const f = rec(raw);
    if (!f || typeof f.itag !== "number" || typeof f.url !== "string" || !f.url) continue;
    if (typeof f.mimeType !== "string" || !f.mimeType.startsWith("audio/")) continue;
    const length = Number(f.contentLength);
    out.push({
      itag: f.itag,
      url: f.url,
      mimeType: f.mimeType,
      bitrate: typeof f.bitrate === "number" ? f.bitrate : Number(f.averageBitrate) || 0,
      ...(length > 0 ? { contentLength: length } : {}),
    });
  }
  return out;
}

export function bestFormat(formats: Format[]): Format | null {
  for (const itag of BEST_ITAGS) {
    const hit = formats.find((f) => f.itag === itag);
    if (hit) return hit;
  }
  return [...formats].sort((a, b) => b.bitrate - a.bitrate)[0] ?? null;
}

export function playabilityProblem(response: unknown): string | null {
  const status = rec(rec(response)?.playabilityStatus);
  if (!status || status.status === "OK") return null;
  return typeof status.reason === "string" && status.reason ? status.reason : `YouTube: ${String(status.status)}`;
}

function expiry(url: string): number | null {
  const expire = Number(new URL(url).searchParams.get("expire"));
  return Number.isFinite(expire) && expire > 0 ? expire * 1000 : null;
}

export function toStream(format: Format): DirectStream {
  const opus = /opus|webm/.test(format.mimeType);
  return {
    url: format.url,
    mimeType: format.mimeType.split(";")[0].trim() || "audio/webm",
    codec: opus ? "opus" : "aac",
    bitrate: format.bitrate || 160_000,
    itag: format.itag,
    ...(format.contentLength ? { contentLength: format.contentLength } : {}),
    expiresAt: expiry(format.url),
  };
}

async function player(profile: Profile, videoId: string, signal?: AbortSignal): Promise<unknown> {
  const call = async (visitorId: string) => {
    const res = await fetch("https://www.youtube.com/youtubei/v1/player?prettyPrint=false", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        accept: "application/json",
        "x-youtube-client-name": profile.clientId,
        "x-youtube-client-version": profile.clientVersion,
        "x-origin": "https://www.youtube.com",
        ...(visitorId ? { "x-goog-visitor-id": visitorId } : {}),
        [USER_AGENT_HEADER]: profile.userAgent,
      },
      body: JSON.stringify({
        videoId,
        contentCheckOk: true,
        racyCheckOk: true,
        context: {
          client: {
            clientName: profile.clientName,
            clientVersion: profile.clientVersion,
            hl: "en",
            gl: "US",
            userAgent: profile.userAgent,
            ...(visitorId ? { visitorData: visitorId } : {}),
            ...profile.extra,
          },
          user: { lockedSafetyMode: false },
        },
      }),
      signal,
    });
    if (!res.ok) throw new Error(`YouTube ответил ${res.status}`);
    return res.json() as Promise<unknown>;
  };
  let response = await call(await visitorData());
  if (rec(rec(response)?.playabilityStatus)?.status === "LOGIN_REQUIRED") {
    visitor = null; // a fresh visitor id often clears the "confirm you're not a bot" answer
    response = await call(await visitorData());
  }
  return response;
}

/** the address answers (through the app's media server, the way the player will ask) */
async function reachable(url: string, signal?: AbortSignal): Promise<void> {
  const res = await fetch(mediaStream(url), { headers: { range: "bytes=0-1" }, signal });
  await res.body?.cancel().catch(() => undefined);
  if (!res.ok) throw new Error(`поток не отдаётся (${res.status})`);
}

// ---- public -----------------------------------------------------------------------------

const resolved = new Map<string, DirectStream>();
let lastGood: Profile | null = null;

function order(): readonly Profile[] {
  return lastGood ? [lastGood, ...PROFILES.filter((p) => p !== lastGood)] : PROFILES;
}

/** audio of a YouTube video, straight from YouTube */
export async function resolveYouTubeAudio(videoId: string, signal?: AbortSignal): Promise<DirectStream> {
  const known = resolved.get(videoId);
  if (known && (known.expiresAt === null || known.expiresAt - Date.now() > 10 * 60_000)) return known;
  const failures: string[] = [];
  for (const profile of order()) {
    try {
      const response = await player(profile, videoId, signal);
      const problem = playabilityProblem(response);
      if (problem) throw new Error(problem);
      const best = bestFormat(audioFormats(response));
      if (!best) throw new Error("нет прямых аудиоформатов");
      const stream = toStream(best);
      await reachable(stream.url, signal);
      lastGood = profile;
      resolved.set(videoId, stream);
      return stream;
    } catch (e) {
      if (signal?.aborted) throw e;
      failures.push(`${profile.clientName}: ${e instanceof Error ? e.message : String(e)}`);
    }
  }
  throw new Error(`YouTube не отдал звук (${failures.join("; ")})`);
}

export function videoIdOf(trackId: string): string | null {
  const id = trackId.replace(/^yt:/, "");
  return /^[A-Za-z0-9_-]{11}$/.test(id) ? id : null;
}
