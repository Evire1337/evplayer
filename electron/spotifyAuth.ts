import { createHash, randomBytes } from "node:crypto";
import http from "node:http";
import { shell } from "electron";

// "Sign in with Spotify": the official authorization-code flow with PKCE, the one Spotify
// made for apps that cannot keep a secret (desktop and phone apps). The user signs in on
// spotify.com in their own browser; Spotify sends the browser back to a small server on
// this computer with a one-time code, which is swapped for a token. No password ever
// passes through EvPlayer.

export const SPOTIFY_REDIRECT_PORT = 43821;
export const SPOTIFY_REDIRECT_URI = `http://127.0.0.1:${SPOTIFY_REDIRECT_PORT}/callback`;
const SCOPES = ["playlist-read-private", "playlist-read-collaborative", "user-library-read", "user-read-private"];

export interface SpotifyTokens {
  accessToken: string;
  refreshToken: string;
  expiresAt: number;
}

const b64url = (buf: Buffer) => buf.toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

export function pkcePair(): { verifier: string; challenge: string } {
  const verifier = b64url(randomBytes(48));
  const challenge = b64url(createHash("sha256").update(verifier).digest());
  return { verifier, challenge };
}

async function token(body: Record<string, string>): Promise<SpotifyTokens> {
  const res = await fetch("https://accounts.spotify.com/api/token", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams(body).toString(),
  });
  const json = (await res.json().catch(() => ({}))) as { access_token?: string; refresh_token?: string; expires_in?: number; error_description?: string; error?: string };
  if (!res.ok || !json.access_token) throw new Error(json.error_description || json.error || `Spotify ответил ${res.status}`);
  return {
    accessToken: json.access_token,
    refreshToken: json.refresh_token ?? body.refresh_token ?? "",
    expiresAt: Date.now() + ((json.expires_in ?? 3600) - 60) * 1000,
  };
}

const PAGE = (title: string, text: string) =>
  `<!doctype html><meta charset="utf-8"><title>EvPlayer</title><body style="margin:0;height:100vh;display:grid;place-items:center;background:#06201d;color:#eef7f4;font:16px system-ui"><div style="text-align:center"><h2>${title}</h2><p style="opacity:.7">${text}</p></div></body>`;

let pending: http.Server | null = null;

/** open the Spotify sign-in page and wait (up to 5 minutes) for the user to come back */
export function signInWithSpotify(clientId: string): Promise<SpotifyTokens> {
  pending?.close();
  const { verifier, challenge } = pkcePair();
  const state = b64url(randomBytes(16));
  return new Promise((resolve, reject) => {
    const server = http.createServer((req, res) => {
      const url = new URL(req.url ?? "/", SPOTIFY_REDIRECT_URI);
      if (url.pathname !== "/callback") {
        res.writeHead(404).end();
        return;
      }
      const finish = (ok: boolean, title: string, text: string) => {
        res.writeHead(ok ? 200 : 400, { "content-type": "text/html; charset=utf-8" }).end(PAGE(title, text));
        clearTimeout(timer);
        server.close();
        if (pending === server) pending = null;
      };
      const code = url.searchParams.get("code");
      if (url.searchParams.get("state") !== state || !code) {
        finish(false, "Вход не выполнен", url.searchParams.get("error") === "access_denied" ? "Вы отменили вход." : "Попробуйте ещё раз из EvPlayer.");
        reject(new Error(url.searchParams.get("error") === "access_denied" ? "Вход в Spotify отменён" : "Spotify не вернул код входа"));
        return;
      }
      token({ grant_type: "authorization_code", code, redirect_uri: SPOTIFY_REDIRECT_URI, client_id: clientId, code_verifier: verifier })
        .then((t) => {
          finish(true, "Готово", "Spotify подключён к EvPlayer. Эту вкладку можно закрыть.");
          resolve(t);
        })
        .catch((e: Error) => {
          finish(false, "Вход не выполнен", e.message);
          reject(e);
        });
    });
    const timer = setTimeout(() => {
      server.close();
      reject(new Error("Вход в Spotify не завершён за 5 минут"));
    }, 5 * 60_000);
    server.on("error", (e) => {
      clearTimeout(timer);
      reject(new Error(`Не получилось открыть порт ${SPOTIFY_REDIRECT_PORT}: ${e.message}`));
    });
    server.listen(SPOTIFY_REDIRECT_PORT, "127.0.0.1", () => {
      pending = server;
      const auth = new URL("https://accounts.spotify.com/authorize");
      auth.search = new URLSearchParams({
        client_id: clientId,
        response_type: "code",
        redirect_uri: SPOTIFY_REDIRECT_URI,
        code_challenge_method: "S256",
        code_challenge: challenge,
        scope: SCOPES.join(" "),
        state,
      }).toString();
      void shell.openExternal(auth.toString());
    });
  });
}

export function refreshSpotify(clientId: string, refreshToken: string): Promise<SpotifyTokens> {
  return token({ grant_type: "refresh_token", refresh_token: refreshToken, client_id: clientId });
}
