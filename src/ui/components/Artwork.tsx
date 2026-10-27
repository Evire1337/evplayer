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

/** a cover image that quietly tries other sizes and disappears instead of showing a broken icon */
export default function Artwork({ src, lazy = true }: { src?: string; lazy?: boolean }) {
  const [list, setList] = useState<string[]>(() => (src ? [src, ...artworkFallbacks(src)] : []));
  useEffect(() => setList(src ? [src, ...artworkFallbacks(src)] : []), [src]);
  if (!list.length) return null;
  return <img src={list[0]} alt="" loading={lazy ? "lazy" : undefined} draggable={false} onError={() => setList((l) => l.slice(1))} />;
}
