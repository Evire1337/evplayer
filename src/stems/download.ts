// A song's sound is fetched whole before it is split. Some services give a stream in pieces
// (YouTube's: about a minute each): a plain request gets the first piece only, and the splitter
// then split just that minute and the tracks ended there. So the file is asked for piece by piece
// (`Range`), following the total the server says, until it is all here.

const RANGE = /bytes\s+(\d+)-(\d+)\/(\d+|\*)/i;
/** no song has this many pieces: a server that never ends must not keep this going */
const MAX_PIECES = 400;

export async function fetchWhole(url: string, signal: AbortSignal, get: typeof fetch = fetch): Promise<ArrayBuffer> {
  const pieces: ArrayBuffer[] = [];
  let have = 0;
  for (let n = 0; n < MAX_PIECES; n++) {
    const res = await get(url, { signal, headers: { Range: `bytes=${have}-` } });
    if (res.status === 416 && have > 0) break;
    if (!res.ok) throw new Error(`звук не скачался (${res.status})`);
    const body = await res.arrayBuffer();
    // the whole file came at once (a server that does not do ranges)
    if (res.status !== 206) return body;
    if (body.byteLength === 0) break;
    pieces.push(body);
    have += body.byteLength;
    const total = Number(RANGE.exec(res.headers.get("Content-Range") ?? "")?.[3]);
    // no total to follow: what came is all there is to know of
    if (!Number.isFinite(total) || have >= total) break;
  }
  if (pieces.length === 1) return pieces[0];
  const whole = new Uint8Array(have);
  let at = 0;
  for (const piece of pieces) {
    whole.set(new Uint8Array(piece), at);
    at += piece.byteLength;
  }
  return whole.buffer;
}

/** a decoded song much shorter than the song is a part of it: it is not split */
export function isWhole(decodedSeconds: number, expectedSeconds: number): boolean {
  if (!(expectedSeconds > 0)) return decodedSeconds > 0;
  return decodedSeconds >= expectedSeconds * 0.9 - 3;
}
