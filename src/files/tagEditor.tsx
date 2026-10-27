import { useEffect, useState } from "react";
import { create } from "zustand";
import type { Track } from "@/types";
import Dialog from "@/ui/components/Dialog";
import { fileToSquareDataUrl, pickImage } from "@/ui/image";
import { toast } from "@/ui/toasts";
import { useLocalFiles } from "./localFiles";
import { useLibrary } from "@/library/store";
import { usePlayer } from "@/player/store";

// Change title, artists, album and cover of one of your files. The file on disk is not
// touched: the changes live in EvPlayer's own index.

const useTagEditor = create<{ track: Track | null }>(() => ({ track: null }));
export const openTagEditor = (track: Track) => useTagEditor.setState({ track });

export default function TagEditorDialog() {
  const track = useTagEditor((s) => s.track);
  const record = useLocalFiles((s) => s.records.find((r) => r.id === track?.id));
  const [title, setTitle] = useState("");
  const [artists, setArtists] = useState("");
  const [album, setAlbum] = useState("");
  const [cover, setCover] = useState<string | null | undefined>(undefined);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!track) return;
    setTitle(record?.title ?? track.title);
    setArtists((record?.artists ?? track.artists.map((a) => a.name)).join(", "));
    setAlbum(record?.album ?? "");
    setCover(undefined);
  }, [track, record]);

  const close = () => useTagEditor.setState({ track: null });
  const preview = cover === undefined ? track?.artwork : cover ?? undefined;

  return (
    <Dialog title="Теги и обложка" open={Boolean(track)} onClose={close}>
      <form
        className="dialog-form"
        onSubmit={async (e) => {
          e.preventDefault();
          if (!track) return;
          setBusy(true);
          try {
            const updated = await window.evplayer?.localFiles.update(track.id, {
              title,
              artists: artists.split(",").map((a) => a.trim()).filter(Boolean),
              album: album || null,
              ...(cover !== undefined ? { cover } : {}),
            });
            await useLocalFiles.getState().refresh();
            if (updated) {
              const fresh = useLocalFiles.getState().records.find((r) => r.id === track.id);
              if (fresh) syncEverywhere(fresh.id);
            }
            toast("Сохранено");
            close();
          } finally {
            setBusy(false);
          }
        }}
      >
        <div className="tag-cover">
          <span className="tag-cover-art">{preview ? <img src={preview} alt="" /> : null}</span>
          <div className="tag-cover-actions">
            <button
              type="button"
              className="btn ghost"
              onClick={async () => {
                const file = await pickImage();
                if (file) setCover(await fileToSquareDataUrl(file, 600));
              }}
            >
              Выбрать обложку
            </button>
            {preview && <button type="button" className="btn ghost" onClick={() => setCover(null)}>Убрать</button>}
          </div>
        </div>
        <label className="field"><span>Название</span><input value={title} onChange={(e) => setTitle(e.target.value)} maxLength={300} /></label>
        <label className="field"><span>Исполнители (через запятую)</span><input value={artists} onChange={(e) => setArtists(e.target.value)} /></label>
        <label className="field"><span>Альбом</span><input value={album} onChange={(e) => setAlbum(e.target.value)} /></label>
        <div className="dialog-actions">
          <button type="button" className="btn ghost" onClick={close}>Отмена</button>
          <button type="submit" className="btn primary" disabled={busy}>Сохранить</button>
        </div>
      </form>
    </Dialog>
  );
}

/** likes, playlists and the queue keep a copy of the track: update those copies too */
async function syncEverywhere(id: string): Promise<void> {
  const { recordToTrack } = await import("./localFiles");
  const record = useLocalFiles.getState().records.find((r) => r.id === id);
  if (!record) return;
  const fresh = recordToTrack(record);
  useLibrary.setState((s) => ({
    likes: s.likes.map((l) => (l.track.id === id ? { ...l, track: fresh } : l)),
    playlists: s.playlists.map((p) => ({ ...p, items: p.items.map((it) => (it.track.id === id ? { ...it, track: fresh } : it)) })),
  }));
  usePlayer.setState((s) => ({ queue: s.queue.map((t) => (t.id === id ? fresh : t)) }));
}
