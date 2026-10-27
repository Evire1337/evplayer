// The app talks to music services straight from its page. A browser engine blocks those
// replies (they are meant for the services' own sites), so requests to the hosts below go
// through the app's own small proxy (evproxy, in src-tauri/src/proxy.rs) instead. The same
// list lives there; anything not on it is refused.

const OPEN_HOSTS = [
  /(^|\.)soundcloud\.com$/,
  /(^|\.)sndcdn\.com$/,
  /(^|\.)soundcloud\.cloud$/,
  /(^|\.)youtube\.com$/,
  /(^|\.)googlevideo\.com$/,
  /(^|\.)ytimg\.com$/,
  /(^|\.)ggpht\.com$/,
  /(^|\.)googleusercontent\.com$/,
  /(^|\.)lrclib\.net$/,
  /(^|\.)genius\.com$/,
  /^api\.deezer\.com$/,
  /^open\.spotify\.com$/,
  /^accounts\.spotify\.com$/,
  /^api\.spotify\.com$/,
  /^api\.music\.yandex\.net$/,
];

export function isOpenHost(url: string): boolean {
  try {
    const u = new URL(url);
    return u.protocol === "https:" && OPEN_HOSTS.some((re) => re.test(u.hostname.toLowerCase()));
  } catch {
    return false;
  }
}

/** how a custom scheme is reached: http://NAME.localhost on Windows and Android, NAME://localhost elsewhere */
export function schemeBase(name: string): string {
  const windowsLike = typeof navigator !== "undefined" && /Windows|Android/i.test(navigator.userAgent);
  return windowsLike ? `http://${name}.localhost/` : `${name}://localhost/`;
}

let proxyBase = "";

/** the proxied address of a service url; other addresses stay as they are */
export function proxied(url: string): string {
  if (!proxyBase || !isOpenHost(url)) return url;
  return proxyBase + encodeURIComponent(url);
}

function urlOf(input: RequestInfo | URL): string {
  if (typeof input === "string") return input;
  if (input instanceof URL) return input.href;
  return input.url;
}

declare global {
  interface Window {
    /** the desktop side's small media server (src-tauri/src/media_server.rs) */
    __EVPLAYER_MEDIA__?: string;
  }
}

/**
 * The address an <audio> element plays a service's song stream from: through the app's
 * media server, which every browser engine can play from (and which keeps the sound
 * readable for the equalizer). Other addresses stay as they are.
 */
export function mediaStream(url: string): string {
  const base = typeof window !== "undefined" ? window.__EVPLAYER_MEDIA__ : undefined;
  if (!base || !isOpenHost(url)) return url;
  return `${base}/remote/${encodeURIComponent(url)}`;
}

/** route the page's fetch and XHR calls to the services through the proxy */
export function installProxy(): void {
  proxyBase = schemeBase("evproxy");
  const nativeFetch = window.fetch.bind(window);
  window.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
    const url = urlOf(input);
    if (!isOpenHost(url)) return nativeFetch(input, init);
    const target = proxied(url);
    if (input instanceof Request) return nativeFetch(new Request(target, input), init);
    return nativeFetch(target, init);
  };
  // hls.js loads playlists and pieces of songs with XMLHttpRequest
  const open = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (this: XMLHttpRequest, method: string, url: string | URL, ...rest: unknown[]) {
    const target = proxied(typeof url === "string" ? url : url.href);
    return (open as (...args: unknown[]) => void).call(this, method, target, ...(rest.length ? rest : [true]));
  } as typeof XMLHttpRequest.prototype.open;
}
