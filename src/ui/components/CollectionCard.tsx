import type { CollectionSummary } from "@/types";
import { useNavigation } from "../navigation";
import { IconDisc } from "./Icons";

export default function CollectionCard({ item }: { item: CollectionSummary }) {
  const go = useNavigation((s) => s.go);
  return (
    <button type="button" className="track-card" onClick={() => go("collection", item.id, item.title)}>
      <span className={`track-card-art${item.kind === "album" ? " is-album" : ""}`}>
        {item.artwork ? <img src={item.artwork} alt="" loading="lazy" /> : <span className="card-placeholder"><IconDisc size={40} /></span>}
      </span>
      <span className="track-card-title">{item.title}</span>
      <span className="track-card-artist">
        {item.kind === "album" ? "Альбом" : "Плейлист"}
        {item.subtitle ? ` · ${item.subtitle.replace(/^(Альбом|Плейлист|Album|Playlist|Сингл|EP)\s*·\s*/i, "")}` : ""}
      </span>
    </button>
  );
}
