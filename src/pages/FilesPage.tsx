import VirtualList from "@/ui/components/VirtualList";
import { useEffect, useMemo, useState } from "react";
import TrackRow from "@/ui/components/TrackRow";
import { IconFolder, IconMusic, IconPlay, IconTrash } from "@/ui/components/Icons";
import { player } from "@/player/engine";
import { formatLabel, recordToTrack, useLocalFiles } from "@/files/localFiles";
import { openMenu } from "@/ui/menu";
import { trackMenuItems } from "@/library/trackMenu";

export default function FilesPage() {
  const { records, folders, loaded, refresh, pick, unwatch, remove } = useLocalFiles();
  const [filter, setFilter] = useState("");
  const [dragging, setDragging] = useState(false);
  const available = Boolean(window.evplayer?.localFiles);

  useEffect(() => {
    void refresh();
  }, [refresh]);
  useEffect(() => window.evplayer?.localFiles.onDragging(setDragging), []);

  const tracks = useMemo(() => records.map(recordToTrack), [records]);
  const visible = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return tracks.map((t, i) => ({ t, i }));
    return tracks.map((t, i) => ({ t, i })).filter(({ t }) => `${t.title} ${t.artists.map((a) => a.name).join(" ")}`.toLowerCase().includes(q));
  }, [tracks, filter]);
  const labels = useMemo(() => new Map(records.map((r) => [r.id, formatLabel(r)])), [records]);
  const unsupported = records.filter((r) => r.unsupported).length;

  return (
    <div className={`page files-page${dragging ? " is-dragging" : ""}`}>
      <header className="pl-hero">
        <span className="pl-cover files-cover"><IconMusic size={64} /></span>
        <div className="pl-hero-text">
          <span className="eyebrow">Ваши файлы</span>
          <h1 className="display">Мои файлы</h1>
          <p className="muted">
            {records.length ? `${records.length} треков` : "Пока пусто"} · MP3, FLAC, M4A/AAC, OGG/Opus, WAV. Файлы остаются на месте — EvPlayer только читает их.
          </p>
          <div className="pl-actions">
            <button type="button" className="round-btn main" aria-label="Слушать всё" disabled={!tracks.length} onClick={() => void player.playQueue(tracks)}>
              <IconPlay size={22} />
            </button>
            <button type="button" className="btn ghost" disabled={!available} onClick={() => void pick("files")}>Добавить файлы</button>
            <button type="button" className="btn ghost" disabled={!available} onClick={() => void pick("folder")}><IconFolder size={16} /> Добавить папку</button>
          </div>
        </div>
      </header>

      {!available && <p className="muted center">Свои файлы работают только в приложении для компьютера.</p>}

      {folders.length > 0 && (
        <section className="row">
          <h2 className="row-title">Папки, за которыми следит EvPlayer</h2>
          <p className="muted small">Новые файлы в этих папках появляются сами, удалённые — исчезают.</p>
          <div className="folder-list">
            {folders.map((f) => (
              <span key={f} className="folder-chip">
                <IconFolder size={15} />
                <span className="folder-path" title={f}>{f}</span>
                <button type="button" className="row-icon" aria-label="Перестать следить" onClick={() => void unwatch(f)}>
                  <IconTrash size={15} />
                </button>
              </span>
            ))}
          </div>
        </section>
      )}

      {unsupported > 0 && <p className="muted small">{unsupported} файл(ов) в формате, который Chromium не умеет воспроизводить (например ALAC) — сконвертируйте их во FLAC.</p>}

      {records.length > 0 && (
        <>
          <div className="pl-tools">
            <input className="filter-input" placeholder="Поиск по файлам" value={filter} onChange={(e) => setFilter(e.target.value)} />
          </div>
          <div className="track-list">
            <VirtualList
              items={visible}
              getKey={({ t }) => t.id}
              render={({ t, i }) => (
              <div
                
                className="file-row"
                onContextMenu={(e) => {
                  e.stopPropagation();
                  openMenu(e, [
                    ...trackMenuItems(t),
                    { label: "Убрать из EvPlayer (файл останется)", icon: <IconTrash size={16} />, danger: true, onSelect: () => void remove([t.id]) },
                  ]);
                }}
              >
                <TrackRow track={t} queue={tracks} index={i} note={labels.get(t.id) || undefined} />
              </div>
              )}
            />
          </div>
        </>
      )}
      {loaded && available && records.length === 0 && <p className="muted center">Перетащите сюда файлы или папку с музыкой.</p>}
    </div>
  );
}
