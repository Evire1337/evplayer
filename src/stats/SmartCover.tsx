import type { Track } from "@/types";
import type { SmartPlaylistKind } from "./listeningStats";
import { SMART_INFO } from "./smart";
import { IconSparkle } from "@/ui/components/Icons";

/** a smart playlist's picture: its first covers like any playlist, with a coloured sparkle badge */
export default function SmartCover({ kind, tracks, size, className = "" }: { kind: SmartPlaylistKind; tracks: Track[]; size?: number; className?: string }) {
  const arts = [...new Set(tracks.map((t) => t.artwork).filter(Boolean))].slice(0, 4) as string[];
  const style = { "--hue": SMART_INFO[kind].hue, ...(size ? { width: size, height: size } : {}) } as React.CSSProperties;
  if (!arts.length) {
    return (
      <span className={`pl-cover smart-cover ${className}`} style={style} aria-hidden="true">
        <IconSparkle size={44} />
      </span>
    );
  }
  return (
    <span className={`pl-cover smart-mosaic ${arts.length >= 4 ? "is-grid" : ""} ${className}`} style={style} aria-hidden="true">
      {(arts.length >= 4 ? arts : arts.slice(0, 1)).map((a) => (
        <img key={a} src={a} alt="" loading="lazy" />
      ))}
      <span className="smart-badge"><IconSparkle size={14} /></span>
    </span>
  );
}
