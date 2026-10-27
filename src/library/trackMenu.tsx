import type { Track } from "@/types";
import { player } from "@/player/engine";
import { useNavigation } from "@/ui/navigation";
import type { MenuItem } from "@/ui/menu";
import { toast } from "@/ui/toasts";
import { IconHeart, IconHeartFill, IconListPlus, IconNext, IconPlus, IconQueue, IconTrash } from "@/ui/components/Icons";
import { isLiked, useLibrary } from "./store";
import { openCreatePlaylist } from "./dialogs";

export interface TrackMenuContext {
  playlistId?: string;
  itemId?: string;
}

export function trackMenuItems(track: Track, ctx: TrackMenuContext = {}): MenuItem[] {
  const lib = useLibrary.getState();
  const liked = isLiked(track.id);
  const items: MenuItem[] = [
    { label: "Играть следующим", icon: <IconNext size={16} />, onSelect: () => player.playNext(track) },
    { label: "Добавить в очередь", icon: <IconQueue size={16} />, onSelect: () => player.addToQueue(track) },
    {
      label: liked ? "Убрать из понравившихся" : "Нравится",
      icon: liked ? <IconHeartFill size={16} /> : <IconHeart size={16} />,
      onSelect: () => lib.toggleLike(track),
    },
    {
      label: "Добавить в плейлист",
      icon: <IconListPlus size={16} />,
      children: [
        { label: "Новый плейлист…", icon: <IconPlus size={16} />, onSelect: () => openCreatePlaylist([track]) },
        ...lib.playlists.map((p) => ({
          label: p.title,
          onSelect: () => {
            const added = useLibrary.getState().addToPlaylist(p.id, [track]);
            toast(added ? `Добавлено в «${p.title}»` : `Уже есть в «${p.title}»`);
          },
        })),
      ],
    },
  ];
  if (ctx.playlistId && ctx.itemId) {
    const { playlistId, itemId } = ctx;
    items.push({ label: "Убрать из этого плейлиста", icon: <IconTrash size={16} />, danger: true, onSelect: () => useLibrary.getState().removeFromPlaylist(playlistId, itemId) });
  }
  return items;
}

export function goToPlaylist(id: string): void {
  useNavigation.getState().go("playlist", id);
}
