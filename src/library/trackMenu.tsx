import type { Track } from "@/types";
import { player } from "@/player/engine";
import { useNavigation } from "@/ui/navigation";
import type { MenuItem } from "@/ui/menu";
import { toast } from "@/ui/toasts";
import { IconChart, IconDisc, IconFolder, IconHeart, IconHeartFill, IconLink, IconListPlus, IconNext, IconPencil, IconPlus, IconQueue, IconRadio, IconTrash, IconUser } from "@/ui/components/Icons";
import { isLiked, useLibrary } from "./store";
import { openCreatePlaylist } from "./dialogs";
import { openTrackStats } from "@/stats/statsDialog";
import { openTagEditor } from "@/files/tagEditor";

export interface TrackMenuContext {
  playlistId?: string;
  itemId?: string;
  /** shown in the queue: remove it from there */
  queueIndex?: number;
}

function artistTarget(track: Track): { id: string; name: string } | null {
  const artist = track.artists[0];
  if (!artist) return null;
  return { id: artist.id, name: artist.name };
}

export function trackMenuItems(track: Track, ctx: TrackMenuContext = {}): MenuItem[] {
  const lib = useLibrary.getState();
  const liked = isLiked(track.id);
  const artist = artistTarget(track);
  const items: MenuItem[] = [
    { label: "Играть следующим", icon: <IconNext size={16} />, onSelect: () => player.playNext(track) },
    { label: "Добавить в очередь", icon: <IconQueue size={16} />, onSelect: () => player.addToQueue(track) },
    { label: "Радио по треку", icon: <IconRadio size={16} />, onSelect: () => void player.startRadio(track) },
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
    { label: "Статистика прослушиваний", icon: <IconChart size={16} />, onSelect: () => openTrackStats(track) },
  ];
  if (artist) {
    items.push({ label: `Исполнитель: ${artist.name}`, icon: <IconUser size={16} />, onSelect: () => useNavigation.getState().go("artist", artist.id, artist.name) });
  }
  if (track.permalink) {
    const permalink = track.permalink;
    items.push({
      label: "Скопировать ссылку",
      icon: <IconLink size={16} />,
      onSelect: () => {
        void navigator.clipboard.writeText(permalink).then(() => toast("Ссылка скопирована"));
      },
    });
    items.push({
      label: track.source === "youtube" ? "Открыть на YouTube" : "Открыть на SoundCloud",
      icon: <IconDisc size={16} />,
      onSelect: () => window.open(permalink, "_blank"),
    });
  }
  if (track.source === "local") {
    items.push({ label: "Изменить теги и обложку", icon: <IconPencil size={16} />, onSelect: () => openTagEditor(track) });
    items.push({ label: "Показать в папке", icon: <IconFolder size={16} />, onSelect: () => void window.evplayer?.localFiles.reveal(track.id) });
  }
  if (ctx.queueIndex !== undefined) {
    const index = ctx.queueIndex;
    items.push({ label: "Убрать из очереди", icon: <IconTrash size={16} />, danger: true, onSelect: () => player.removeFromQueue(index) });
  }
  if (ctx.playlistId && ctx.itemId) {
    const { playlistId, itemId } = ctx;
    items.push({ label: "Убрать из этого плейлиста", icon: <IconTrash size={16} />, danger: true, onSelect: () => useLibrary.getState().removeFromPlaylist(playlistId, itemId) });
  }
  return items;
}

export function goToPlaylist(id: string): void {
  useNavigation.getState().go("playlist", id);
}
