import type { Track } from "@/types";

// A whole playlist as a line of text: send it in any chat, paste it into "Import" on another
// computer. EVP2.<base64url deflate-raw json>, or EVP1.<base64url json> as a fallback.

export interface SharedPlaylist {
  title: string;
  description?: string;
  icon?: string;
  tracks: Track[];
  /** a copy of someone's «Понравившиеся»: pasting it adds to your likes */
  likes?: boolean;
}

interface Wire {
  v: 2;
  n: string;
  d?: string;
  c?: string;
  /** 1: the list is «Понравившиеся» */
  l?: 1;
  t: Array<[string, string, string, number, string?, Array<[string, string]>?]>;
}

const MAX_ICON = 60_000;
const SOURCES = { s: "soundcloud", y: "youtube", l: "local" } as const;
const SHORT = { soundcloud: "s", youtube: "y", local: "l" } as const;

/** characters that are not part of the code but get into a text file or a chat: byte-order mark, zero-width spaces */
const INVISIBLE = /[\u200B-\u200D\u2060\uFEFF]/g;
const CODE_START = /EVP[12]\./g;
const CODE_BODY = /^[A-Za-z0-9_\-+/=]+$/;

/**
 * Every playlist code in a pasted text, each as one clean string. A text file with several codes
 * (one per line, maybe with names or empty lines between) gives all of them; a single code that
 * was wrapped over several lines gets joined again.
 */
export function extractPlaylistCodes(text: string): string[] {
  const clean = text.replace(INVISIBLE, "");
  const starts = [...clean.matchAll(CODE_START)].map((m) => m.index ?? 0);
  return starts.map((at, i) => {
    const chunk = clean.slice(at, starts[i + 1] ?? clean.length);
    const head = chunk.slice(0, 5);
    const tokens = chunk.slice(5).split(/\s+/).filter(Boolean);
    // one code in the text: all that follows it that looks like code belongs to it (a wrapped line);
    // several: each line is its own code, and what stands between them is a name or a note
    const parts: string[] = [];
    for (const token of tokens) {
      if (!CODE_BODY.test(token)) break;
      parts.push(token);
      if (starts.length > 1) break;
    }
    return head + parts.join("");
  });
}

export const looksLikePlaylistCode = (text: string) => extractPlaylistCodes(text).length > 0;

function toB64(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function fromB64(text: string): Uint8Array {
  const n = text.replace(/\s+/g, "").replace(/=+$/, "").replace(/-/g, "+").replace(/_/g, "/");
  const bin = atob(n + "=".repeat((4 - (n.length % 4)) % 4));
  return Uint8Array.from(bin, (c) => c.charCodeAt(0));
}

async function pipe(bytes: Uint8Array, stream: CompressionStream | DecompressionStream): Promise<Uint8Array> {
  const writer = stream.writable.getWriter();
  void writer.write(bytes as BufferSource);
  void writer.close();
  return new Uint8Array(await new Response(stream.readable).arrayBuffer());
}

export async function encodePlaylistCode(p: SharedPlaylist): Promise<string> {
  const wire: Wire = {
    v: 2,
    n: p.title,
    ...(p.description ? { d: p.description } : {}),
    ...(p.icon && p.icon.length <= MAX_ICON ? { c: p.icon } : {}),
    ...(p.likes ? { l: 1 as const } : {}),
    t: p.tracks.map((t) => [SHORT[t.source], t.id, t.title, t.durationMs, t.artwork ?? "", t.artists.map((a) => [a.id, a.name] as [string, string])]),
  };
  const raw = new TextEncoder().encode(JSON.stringify(wire));
  if (typeof CompressionStream !== "undefined") {
    try {
      return `EVP2.${toB64(await pipe(raw, new CompressionStream("deflate-raw")))}`;
    } catch {
      // fall back to the plain form
    }
  }
  return `EVP1.${toB64(raw)}`;
}

export async function decodePlaylistCode(code: string): Promise<SharedPlaylist> {
  const first = extractPlaylistCodes(code)[0];
  const m = first ? /^(EVP[12])\.([A-Za-z0-9_\-+/=]+)$/.exec(first) : null;
  if (!m) throw new Error("Это не код плейлиста");
  let bytes = fromB64(m[2]);
  if (m[1] === "EVP2") bytes = await pipe(bytes, new DecompressionStream("deflate-raw"));
  const wire = JSON.parse(new TextDecoder().decode(bytes)) as Wire;
  if (!wire || typeof wire.n !== "string" || !Array.isArray(wire.t)) throw new Error("Код повреждён");
  const tracks = wire.t.flatMap((row): Track[] => {
    const [src, id, title, duration, art, artists] = row;
    const source = SOURCES[src as keyof typeof SOURCES];
    if (!source || typeof id !== "string" || typeof title !== "string") return [];
    return [{ id, source, title, durationMs: Number(duration) || 0, ...(art ? { artwork: art } : {}), artists: (artists ?? []).map(([aid, name]) => ({ id: aid, name })) }];
  });
  return { title: wire.n, ...(wire.d ? { description: wire.d } : {}), ...(wire.c ? { icon: wire.c } : {}), ...(wire.l === 1 ? { likes: true } : {}), tracks };
}
