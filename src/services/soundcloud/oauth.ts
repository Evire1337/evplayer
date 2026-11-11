import { useSettings } from "@/settings/store";
import { guardSoundCloud } from "@/services/region";

// Signing in to SoundCloud with its official OAuth, and calls to the write side of its API
// (api.soundcloud.com) on behalf of that account. Tokens live about an hour and a refresh
// token works once, so a refresh hands back a new pair: only one refresh runs at a time and
// the new pair is saved before anything else uses it.

const API = "https://api.soundcloud.com";

export class AuthError extends Error {}

export class ApiError extends Error {
  constructor(readonly status: number, message: string) {
    super(message);
  }
}

function appKeys(): { id: string; secret: string } {
  const s = useSettings.getState();
  const id = s.soundcloudClientId.trim();
  const secret = s.soundcloudClientSecret.trim();
  if (!id || !secret) throw new AuthError("Нужны client_id и client_secret вашего приложения SoundCloud");
  return { id, secret };
}

export async function signInSoundCloud(): Promise<void> {
  const bridge = window.evplayer?.soundcloudAuth;
  if (!bridge) throw new Error("Вход в SoundCloud работает только в приложении для компьютера");
  const { id, secret } = appKeys();
  const tokens = await bridge.signIn(id, secret);
  useSettings.getState().set({ soundcloudLogin: { id: "", name: "SoundCloud", ...tokens } });
  const me = await call<{ id?: number; urn?: string; username?: string; avatar_url?: string }>("GET", "/me");
  const login = useSettings.getState().soundcloudLogin;
  if (login) {
    useSettings.getState().set({
      soundcloudLogin: { ...login, id: String(me.id ?? me.urn ?? ""), name: me.username || "SoundCloud", avatar: me.avatar_url || undefined },
    });
  }
}

export function signOutSoundCloud(): void {
  useSettings.getState().set({ soundcloudLogin: null, soundcloudFullSync: false });
}

let refreshing: Promise<string> | null = null;

async function accessToken(force = false): Promise<string> {
  const login = useSettings.getState().soundcloudLogin;
  if (!login) throw new AuthError("Вы не вошли в SoundCloud");
  if (!force && Date.now() < login.expiresAt) return login.accessToken;
  refreshing ??= (async () => {
    try {
      const current = useSettings.getState().soundcloudLogin;
      const bridge = window.evplayer?.soundcloudAuth;
      if (!current || !bridge) throw new AuthError("Вы не вошли в SoundCloud");
      const { id, secret } = appKeys();
      const fresh = await bridge.refresh(id, secret, current.refreshToken);
      useSettings.getState().set({ soundcloudLogin: { ...current, ...fresh } });
      return fresh.accessToken;
    } catch (e) {
      throw new AuthError(`Вход в SoundCloud устарел, войдите заново в настройках${e instanceof Error && e.message ? ` (${e.message})` : ""}`);
    } finally {
      refreshing = null;
    }
  })();
  return refreshing;
}

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** one call to the API; waits out "too many requests" and renews an expired token once */
export async function call<T = unknown>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  // an account asked from a country SoundCloud blocks can stay marked as that country's
  await guardSoundCloud();
  let renewed = false;
  for (let attempt = 0; ; attempt++) {
    const res = await fetch(`${API}${path}`, {
      method,
      headers: {
        authorization: `OAuth ${await accessToken()}`,
        accept: "application/json; charset=utf-8",
        ...(body === undefined ? {} : { "content-type": "application/json" }),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal,
    });
    if (res.status === 429 && attempt < 4) {
      await wait(Math.min(60, Number(res.headers.get("retry-after") ?? 5) || 5) * 1000);
      continue;
    }
    if (res.status === 401 && !renewed) {
      renewed = true;
      await accessToken(true);
      continue;
    }
    if (res.status === 401) throw new AuthError("SoundCloud не принял вход, войдите заново в настройках");
    if (!res.ok) throw new ApiError(res.status, `SoundCloud ответил ${res.status}`);
    const text = await res.text();
    return (text ? JSON.parse(text) : {}) as T;
  }
}
