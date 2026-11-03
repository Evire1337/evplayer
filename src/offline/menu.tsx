import type { MenuItem } from "@/ui/menu";
import { toast } from "@/ui/toasts";
import { IconDownload, IconPlus, IconTrash } from "@/ui/components/Icons";
import { openDownloadDialog } from "./dialog";
import { forgetTracks, keepTracks } from "./keep";
import { scopeStatus, scopeTracks } from "./scopes";
import { offlineSupported, useKeepLists, useOffline } from "./store";

/**
 * The offline entries of the menu of a list (the likes or a playlist), next to "copy the
 * code": download it, download it together with other lists, or delete the copies.
 */
export function offlineMenuItems(scope: string): MenuItem[] {
  if (!offlineSupported()) return [];
  const status = scopeStatus(scope);
  if (status.total === 0) return [];
  const items: MenuItem[] = [];
  if (status.kept < status.total) {
    items.push({
      label: status.kept ? `Докачать остальное (${status.total - status.kept})` : "Скачать для офлайн",
      icon: <IconDownload size={16} />,
      onSelect: () => {
        useKeepLists.getState().add([scope]);
        const added = keepTracks(scopeTracks(scope));
        if (added) toast(`Скачиваю: ${added}`);
      },
    });
  }
  items.push({ label: "Скачать вместе с другими…", icon: <IconPlus size={16} />, onSelect: () => openDownloadDialog([scope]) });
  if (status.kept > 0 || useKeepLists.getState().has(scope)) {
    items.push({
      label: "Удалить скачанное",
      icon: <IconTrash size={16} />,
      danger: true,
      onSelect: () => {
        useKeepLists.getState().remove([scope]);
        const entries = useOffline.getState().entries;
        // a song that another list still keeps offline stays
        const others = new Set(useKeepLists.getState().lists.flatMap(scopeTracks).map((t) => t.id));
        const ids = scopeTracks(scope).filter((t) => t.id in entries && !others.has(t.id)).map((t) => t.id);
        void forgetTracks(ids).then(() => toast(ids.length ? `Удалено копий: ${ids.length}` : "Эти песни нужны другим спискам, оставил"));
      },
    });
  }
  return items;
}
