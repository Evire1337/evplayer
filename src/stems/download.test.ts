import { describe, expect, it } from "vitest";
import { fetchWhole, isWhole } from "./download";

/** a server that gives a file of `size` bytes in pieces of at most `piece` bytes (as YouTube's does), or whole if piece is 0 */
function server(size: number, piece: number, withTotal = true) {
  const asked: string[] = [];
  const file = Uint8Array.from({ length: size }, (_, i) => i % 251);
  const get = (async (_url: string, init: { headers?: { Range?: string } }) => {
    const range = init.headers?.Range ?? "";
    asked.push(range);
    const start = Number(/bytes=(\d+)-/.exec(range)?.[1] ?? 0);
    if (piece === 0) return { status: 200, ok: true, headers: { get: () => null }, arrayBuffer: async () => file.buffer.slice(0) };
    if (start >= size) return { status: 416, ok: false, headers: { get: () => null }, arrayBuffer: async () => new ArrayBuffer(0) };
    const end = Math.min(size, start + piece) - 1;
    return {
      status: 206,
      ok: true,
      headers: { get: () => `bytes ${start}-${end}/${withTotal ? size : "*"}` },
      arrayBuffer: async () => file.slice(start, end + 1).buffer,
    };
  }) as unknown as typeof fetch;
  return { get, asked, file };
}

describe("fetching a whole song", () => {
  it("joins the pieces a server gives, in order, until the total is reached", async () => {
    const s = server(2_500_000, 1_048_576);
    const data = new Uint8Array(await fetchWhole("x", new AbortController().signal, s.get));
    expect(data.length).toBe(2_500_000);
    expect(data.every((v, i) => v === s.file[i])).toBe(true);
    expect(s.asked).toEqual(["bytes=0-", "bytes=1048576-", "bytes=2097152-"]);
  });

  it("takes a file that comes whole from a server without ranges", async () => {
    const s = server(1000, 0);
    expect((await fetchWhole("x", new AbortController().signal, s.get)).byteLength).toBe(1000);
    expect(s.asked.length).toBe(1);
  });

  it("stops at what it has where the server does not say the total, and where the file ends", async () => {
    expect((await fetchWhole("x", new AbortController().signal, server(3000, 1000, false).get)).byteLength).toBe(1000);
    // a total that the last piece reaches exactly does not ask for one more
    const s = server(2000, 1000);
    expect((await fetchWhole("x", new AbortController().signal, s.get)).byteLength).toBe(2000);
    expect(s.asked.length).toBe(2);
  });

  it("fails when the server refuses", async () => {
    const get = (async () => ({ status: 403, ok: false, headers: { get: () => null }, arrayBuffer: async () => new ArrayBuffer(0) })) as unknown as typeof fetch;
    await expect(fetchWhole("x", new AbortController().signal, get)).rejects.toThrow(/403/);
  });
});

describe("a song that was fetched whole", () => {
  it("is as long as the song, give or take what a service rounds", () => {
    expect(isWhole(140.1, 140)).toBe(true);
    expect(isWhole(137, 140)).toBe(true);
  });

  it("is not a minute of a song of two and a half", () => {
    expect(isWhole(59, 140)).toBe(false);
  });

  it("is anything that plays when the length of the song is not known", () => {
    expect(isWhole(59, 0)).toBe(true);
    expect(isWhole(0, 0)).toBe(false);
  });
});
