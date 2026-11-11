import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useSettings } from "@/settings/store";
import { allowed, forgetCountry, guardSoundCloud, lookupCountry, normalizeCountry, parseCountry, RegionBlockedError } from "./region";

const TRACE = (loc: string) => `fl=1\nip=203.0.113.9\nloc=${loc}\ntls=TLSv1.3\n`;

describe("country of the connection", () => {
  it("reads the country out of the service's answer, and a typed code", () => {
    expect(parseCountry(TRACE("PL"))).toBe("PL");
    expect(parseCountry("fl=1\nip=1.1.1.1\n")).toBeNull();
    expect(normalizeCountry(" ru ")).toBe("RU");
    expect(normalizeCountry("RUS")).toBeNull();
    expect(normalizeCountry("1")).toBeNull();
  });

  it("holds SoundCloud back only from the chosen countries, and only when asked to", () => {
    const on = { regionGuard: true, regionCountries: ["RU", "BY"] };
    expect(allowed("RU", on)).toBe(false);
    expect(allowed("DE", on)).toBe(true);
    expect(allowed("RU", { ...on, regionGuard: false })).toBe(true);
    // no answer from the service: nothing is held back
    expect(allowed(null, on)).toBe(true);
  });
});

describe("the guard before a request to SoundCloud", () => {
  let answer: string | Error;
  const fetchMock = vi.fn(async () => {
    if (answer instanceof Error) throw answer;
    return new Response(answer, { status: 200 });
  });

  beforeEach(() => {
    forgetCountry();
    fetchMock.mockClear();
    vi.stubGlobal("fetch", fetchMock);
    useSettings.setState({ regionGuard: true, regionCountries: ["RU"] });
  });
  afterEach(() => vi.unstubAllGlobals());

  it("stops a request from a chosen country, and says which", async () => {
    answer = TRACE("RU");
    await expect(guardSoundCloud()).rejects.toBeInstanceOf(RegionBlockedError);
    await expect(guardSoundCloud()).rejects.toThrow(/RU/);
  });

  it("lets a request from another country through, asking the service once while the answer is fresh", async () => {
    answer = TRACE("DE");
    await guardSoundCloud();
    await guardSoundCloud();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    // a switched-off VPN: the connection changed, the old answer goes
    forgetCountry();
    answer = TRACE("RU");
    await expect(guardSoundCloud()).rejects.toBeInstanceOf(RegionBlockedError);
  });

  it("does not ask at all when the guard is off or no country is chosen", async () => {
    answer = TRACE("RU");
    useSettings.setState({ regionGuard: false });
    await guardSoundCloud();
    useSettings.setState({ regionGuard: true, regionCountries: [] });
    await guardSoundCloud();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("goes on when the service does not answer, and does not ask again at once", async () => {
    answer = new Error("offline");
    await guardSoundCloud();
    expect(await lookupCountry()).toBeNull();
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
