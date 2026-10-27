import { useEffect, useState } from "react";

/** other addresses to try when a cover does not load: a smaller size, then nothing */
export function artworkFallbacks(url: string): string[] {
  const out: string[] = [];
  if (/sndcdn\.com/.test(url)) {
    for (const size of ["-t300x300", "-large", "-original"]) {
      const next = url.replace(/-(t\d+x\d+|large|crop|original)(?=\.\w+$)/, size);
      if (next !== url && !out.includes(next)) out.push(next);
    }
  } else if (/i\.ytimg\.com\/vi\/[^/]+\//.test(url)) {
    const id = /\/vi\/([^/]+)\//.exec(url)![1];
    for (const name of ["hqdefault.jpg", "mqdefault.jpg"]) {
      const next = `https://i.ytimg.com/vi/${id}/${name}`;
      if (next !== url) out.push(next);
    }
  } else if (/=w\d+-h\d+/.test(url)) {
    out.push(url.replace(/=w\d+-h\d+/, "=w226-h226"));
  }
  return out;
}

/**
 * A smaller copy of a cover for small places: a 48 px row does not need a 500 px picture,
 * and every decoded picture costs memory. "row" ≈ 100 px, "card" ≈ 300 px.
 */
export function sizedArtwork(url: string, size: "row" | "card" | "full"): string {
  if (size === "full") return url;
  if (/sndcdn\.com/.test(url)) return url.replace(/-(t\d+x\d+|large|crop|original)(?=\.\w+$)/, size === "row" ? "-large" : "-t300x300");
  if (/i\.ytimg\.com\/vi\/[^/]+\//.test(url)) {
    const id = /\/vi\/([^/]+)\//.exec(url)![1];
    // mqdefault is 16:9 without black bars (hqdefault is 4:3 with bars, which would show in a square)
    return `https://i.ytimg.com/vi/${id}/mqdefault.jpg`;
  }
  if (/=w\d+-h\d+/.test(url)) return url.replace(/=w\d+-h\d+/, size === "row" ? "=w120-h120" : "=w300-h300");
  if (/=s\d+/.test(url) && /(ggpht|googleusercontent)\.com/.test(url)) return url.replace(/=s\d+/, size === "row" ? "=s120" : "=s300");
  return url;
}

function candidates(src: string | undefined, size: "row" | "card" | "full"): string[] {
  if (!src) return [];
  const first = sizedArtwork(src, size);
  return [...new Set([first, src, ...artworkFallbacks(src)])];
}

/** a cover image that quietly tries other sizes and disappears instead of showing a broken icon */
export default function Artwork({ src, lazy = true, size = "full" }: { src?: string; lazy?: boolean; size?: "row" | "card" | "full" }) {
  const [list, setList] = useState<string[]>(() => candidates(src, size));
  useEffect(() => setList(candidates(src, size)), [src, size]);
  if (!list.length) return null;
  return <img src={list[0]} alt="" loading={lazy ? "lazy" : undefined} decoding="async" draggable={false} onError={() => setList((l) => l.slice(1))} />;
}
