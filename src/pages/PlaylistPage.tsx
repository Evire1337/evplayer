import { useMemo, useState } from "react";
import type { Track } from "@/types";
import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import { useLibrary } from "@/library/store";
import PlaylistCover from "@/library/Cover";
import { encodePlaylistCode } from "@/library/playlistCode";
import { useNavigation } from "@/ui/navigation";
import { formatDate } from "@/ui/dates";
import { openMenu } from "@/ui/menu";
import { toast } from "@/ui/toasts";
import { fileToSquareDataUrl, pickImage } from "@/ui/image";
import TrackRow from "@/ui/components/TrackRow";
import Dialog from "@/ui/components/Dialog";
import { IconImage, IconMore, IconPencil, IconPlay, IconShare, IconShuffle, IconTrash } from "@/ui/components/Icons";
import { songs } from "./LibraryPage";

interface Row {
  key: string;
  track: Track;
  addedAt: string;
  itemId?: string;
}

export default function PlaylistPage() {
  const id = useNavigation((s) => s.playlistId);
  const go = useNavigation((s) => s.go);
  const isLikes = id === "likes";
  const likes = useLibrary((s) => s.likes);
  const playlist = useLibrary((s) => s.playlists.find((p) => p.id === id));
  const [filter, setFilter] = useState("");
  const [drag, setDrag] = useState<{ from: number; over: number; before: boolean } | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const rows: Row[] = useMemo(() => {
    if (isLikes) return likes.map((l) => ({ key: l.track.id, track: l.track, addedAt: l.addedAt }));
    return (playlist?.items ?? []).map((i) => ({ key: i.id, track: i.track, addedAt: i.addedAt, itemId: i.id }));
  }, [isLikes, likes, playlist]);

  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return rows;
    return rows.filter((r) => `${r.track.title} ${r.track.artists.map((a) => a.name).join(" ")}`.toLowerCase().includes(q));
  }, [rows, filter]);
  const tracks = shown.map((r) => r.track);
  const canReorder = !filter.trim();

  if (!isLikes && !playlist) {
    return (
      <div className="page">
        <p className="muted">Плейлист не найден.</p>
      </div>
    );
  }

  const title = isLikes ? "Понравившиеся" : playlist!.title;

  const share = async () => {
    if (!playlist) return;
    try {
      const code = await encodePlaylistCode({ title: playlist.title, description: playlist.description, icon: playlist.icon, tracks: playlist.items.map((i) => i.track) });
      await navigator.clipboard.writeText(code);
      toast("Код плейлиста скопирован — отправьте его другу");
    } catch {
      toast("Не получилось скопировать код", "error");
    }
  };

  const changeIcon = async () => {
    const file = await pickImage();
    if (!file || !playlist) return;
    try {
      useLibrary.getState().updatePlaylist(playlist.id, { icon: await fileToSquareDataUrl(file) });
    } catch {
      toast("Не получилось открыть картинку", "error");
    }
  };

  const drop = () => {
    if (!drag) return;
    const to = drag.over + (drag.before ? 0 : 1) - (drag.from < drag.over + (drag.before ? 0 : 1) ? 1 : 0);
    const row = rows[drag.from];
    if (isLikes) useLibrary.getState().moveLike(row.track.id, to);
    else if (row.itemId && playlist) useLibrary.getState().moveInPlaylist(playlist.id, row.itemId, to);
    setDrag(null);
  };

  return (
    <div className="page">
      <header className="pl-hero">
        {isLikes ? <PlaylistCover liked size={210} /> : <PlaylistCover playlist={playlist} size={210} />}
        <div className="pl-hero-text">
          <span className="eyebrow">{isLikes ? "Ваши лайки" : "Плейлист"}</span>
          {renaming && playlist ? (
            <form
              onSubmit={(e) => {
                e.preventDefault();
                const value = new FormData(e.currentTarget).get("title");
                useLibrary.getState().updatePlaylist(playlist.id, { title: String(value ?? "") });
                setRenaming(false);
              }}
            >
              <input id="rename" name="title" className="rename-input" defaultValue={playlist.title} autoFocus maxLength={60} onBlur={(e) => e.currentTarget.form?.requestSubmit()} />
            </form>
          ) : (
            <h1 className="display">{title}</h1>
          )}
          {playlist?.description && <p className="muted">{playlist.description}</p>}
          <span className="muted small">
            {songs(rows.length)}
            {playlist ? ` · создан ${formatDate(playlist.createdAt)}` : ""}
          </span>
          <div className="pl-actions">
            <button type="button" className="btn primary" disabled={!tracks.length} onClick={() => void player.playQueue(tracks, 0)}>
              <IconPlay size={16} /> Слушать
            </button>
            <button type="button" className="btn ghost" disabled={!tracks.length} onClick={() => {
                usePlayer.setState({ shuffle: true });
                void player.playQueue(tracks, Math.floor(Math.random() * tracks.length));
              }}>
              <IconShuffle size={16} /> Вперемешку
            </button>
            {playlist && (
              <button
                type="button"
                className="round-btn soft"
                aria-label="Ещё"
                onClick={(e) => {
                  const r = e.currentTarget.getBoundingClientRect();
                  openMenu({ clientX: r.left, clientY: r.bottom + 6 }, [
                    { label: "Поделиться кодом", icon: <IconShare size={16} />, onSelect: () => void share() },
                    { label: "Сменить обложку", icon: <IconImage size={16} />, onSelect: () => void changeIcon() },
                    ...(playlist.icon ? [{ label: "Убрать обложку", onSelect: () => useLibrary.getState().updatePlaylist(playlist.id, { icon: "" }) }] : []),
                    { label: "Переименовать", icon: <IconPencil size={16} />, onSelect: () => setRenaming(true) },
                    { label: "Удалить плейлист", icon: <IconTrash size={16} />, danger: true, onSelect: () => setConfirmDelete(true) },
                  ]);
                }}
              >
                <IconMore size={18} />
              </button>
            )}
          </div>
        </div>
      </header>

      <div className="pl-tools">
        <input id="pl-filter" className="filter-input" value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Найти в плейлисте" />
        {canReorder && rows.length > 1 && <span className="muted small">Перетаскивайте треки, чтобы поменять порядок</span>}
      </div>

      <div className="track-list" onDragEnd={() => setDrag(null)}>
        <div className="track-head" aria-hidden="true">
          <span>Трек</span>
          <span>Добавлен</span>
          <span />
          <span>Время</span>
          <span />
        </div>
        {shown.map((r, i) => (
          <TrackRow
            key={r.key}
            track={r.track}
            queue={tracks}
            index={i}
            addedAt={r.addedAt}
            menu={r.itemId && playlist ? { playlistId: playlist.id, itemId: r.itemId } : undefined}
            draggable={canReorder}
            onDragStart={() => setDrag({ from: i, over: i, before: true })}
            onDragOverRow={(before) => setDrag((d) => (d ? { ...d, over: i, before } : d))}
            onDrop={drop}
            dropHint={drag && drag.over === i && drag.from !== i ? (drag.before ? "before" : "after") : null}
          />
        ))}
        {rows.length === 0 && <p className="muted center">{isLikes ? "Нажмите ♥ у любого трека, и он появится здесь." : "Пока пусто. Правый клик по треку → «Добавить в плейлист»."}</p>}
      </div>

      <Dialog title="Удалить плейлист?" open={confirmDelete} onClose={() => setConfirmDelete(false)}>
        <p className="muted">«{title}» удалится вместе со списком треков. Сами треки останутся там, где были.</p>
        <div className="dialog-actions">
          <button type="button" className="btn ghost" onClick={() => setConfirmDelete(false)}>Отмена</button>
          <button
            type="button"
            className="btn danger"
            onClick={() => {
              if (playlist) useLibrary.getState().deletePlaylist(playlist.id);
              setConfirmDelete(false);
              go("library");
            }}
          >
            Удалить
          </button>
        </div>
      </Dialog>
    </div>
  );
}
