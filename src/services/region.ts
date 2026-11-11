import { create } from "zustand";
import { useSettings } from "@/settings/store";

// SoundCloud decides from the address a request comes from which country the listener is in:
// from a country where it has blocks (Russia, for one) it marks tracks "not available in your
// country", and a signed-in account can stay marked after the address changes. So the app
// asks which country its connection is in (a neutral service, through the same route as the
// requests, a VPN included) and does not talk to SoundCloud from the countries the person
// chose. Songs then play from their YouTube Music copy. Off in Settings → Services.

export const COUNTRY_URL = "https://www.cloudflare.com/cdn-cgi/trace";
/** the answer is trusted this long (a VPN may be switched on or off any time) */
const FRESH_MS = 30_000;
/** a failed lookup is not repeated for this long */
const FAILED_MS = 3_000;
/** countries offered as ticks; any other two-letter code can be added */
export const COMMON_COUNTRIES = ["RU", "UA", "BY", "KZ"] as const;

/** "loc=PL" from the service's plain-text answer */
export function parseCountry(trace: string): string | null {
  return /^loc=([A-Z]{2})$/m.exec(trace)?.[1] ?? null;
}

/** a typed country: two letters, upper case; null when it is not one */
export function normalizeCountry(input: string): string | null {
  const code = input.trim().toUpperCase();
  return /^[A-Z]{2}$/.test(code) ? code : null;
}

interface RegionState {
  /** the country of the connection at the last look, null while unknown */
  country: string | null;
  at: number;
  failedAt: number;
}

export const useRegion = create<RegionState>(() => ({ country: null, at: 0, failedAt: 0 }));

let pending: Promise<string | null> | null = null;

/** which country the connection is in now (asked again when the last answer is old) */
export function lookupCountry(force = false): Promise<string | null> {
  const s = useRegion.getState();
  if (!force && s.country && Date.now() - s.at < FRESH_MS) return Promise.resolve(s.country);
  if (!force && !s.country && Date.now() - s.failedAt < FAILED_MS) return Promise.resolve(null);
  pending ??= (async () => {
    try {
      const res = await fetch(COUNTRY_URL, { cache: "no-store" });
      const country = res.ok ? parseCountry(await res.text()) : null;
      useRegion.setState(country ? { country, at: Date.now() } : { country: null, failedAt: Date.now() });
      return country;
    } catch {
      useRegion.setState({ country: null, failedAt: Date.now() });
      return null;
    } finally {
      pending = null;
    }
  })();
  return pending;
}

/** the connection changed (a VPN, another network): the old answer is no good */
export function forgetCountry(): void {
  useRegion.setState({ country: null, at: 0, failedAt: 0 });
}

if (typeof window !== "undefined") {
  window.addEventListener("online", forgetCountry);
  window.addEventListener("offline", forgetCountry);
}

/** SoundCloud was not asked: the connection is in a country the person chose to keep it out of */
export class RegionBlockedError extends Error {
  constructor(readonly country: string) {
    super(`SoundCloud не трогаю с адреса страны ${country}: включите VPN другой страны или отключите защиту в Настройки → Сервисы`);
  }
}

/** whether SoundCloud may be asked from this country, by the settings */
export function allowed(country: string | null, s: { regionGuard: boolean; regionCountries: string[] }): boolean {
  // the country is unknown (no answer from the service): nothing is held back, the requests
  // would fail on their own when there is no connection
  return !s.regionGuard || !country || !s.regionCountries.includes(country);
}

/** called before every request to SoundCloud */
export async function guardSoundCloud(): Promise<void> {
  const s = useSettings.getState();
  if (!s.regionGuard || !s.regionCountries.length) return;
  const country = await lookupCountry();
  if (!allowed(country, s)) throw new RegionBlockedError(country!);
}
