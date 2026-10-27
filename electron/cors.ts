// The app talks to music services straight from its own page. Browsers would block those
// replies (they are meant for the services' own sites), so the main process marks replies
// from the hosts we use as readable by the app. Nothing else is touched.

const OPEN_HOSTS = [
  /(^|\.)soundcloud\.com$/,
  /(^|\.)sndcdn\.com$/,
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
];

export function isOpenHost(url: string): boolean {
  try {
    const host = new URL(url).hostname.toLowerCase();
    return OPEN_HOSTS.some((re) => re.test(host));
  } catch {
    return false;
  }
}

const CORS_NAMES = new Set([
  "access-control-allow-origin",
  "access-control-allow-headers",
  "access-control-allow-methods",
  "access-control-allow-credentials",
  "access-control-expose-headers",
]);

export function openCorsHeaders(
  headers: Record<string, string[]> | undefined,
): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  for (const [name, value] of Object.entries(headers ?? {})) {
    if (!CORS_NAMES.has(name.toLowerCase())) out[name] = value;
  }
  out["access-control-allow-origin"] = ["*"];
  out["access-control-allow-headers"] = ["*"];
  out["access-control-allow-methods"] = ["GET, HEAD, POST, OPTIONS"];
  out["access-control-expose-headers"] = ["*"];
  return out;
}
