import { useState } from "react";
import { create } from "zustand";
import Dialog from "@/ui/components/Dialog";
import { toast } from "@/ui/toasts";
import { useNavigation } from "@/ui/navigation";
import { parseSpotifyExport, readExportFiles, type SpotifyExportPlaylist } from "./spotifyExport";
import { matchTracks } from "./matchTracks";
import { useLibrary } from "./store";
import { useImportProgress } from "./importProgress";

// Import from Spotify's "Download your data" export: likes, playlists and the most played
// songs from the history. Works for any account, Premium or not.

const useSpotifyDialog = create<{ open: boolean }>(() => ({ open: false }));
export const openSpotifyExport = () => useSpotifyDialog.setState({ open: true });

const LABELS = { liked: "Любимые из Spotify", top: "Самое слушаемое в Spotify", importedFrom: "Импортировано из Spotify" };

async function importPlaylists(chosen: SpotifyExportPlaylist[]): Promise<void> {
  const controller = new AbortController();
  const progress = useImportProgress.getState();
  progress.start("Spotify: начинаю…", () => controller.abort());
  let lastId = "";
  try {
    for (let i = 0; i < chosen.length; i++) {
      const p = chosen[i];
      const prefix = chosen.length > 1 ? `${i + 1}/${chosen.length} · ` : "";
      const tracks = await matchTracks(p.tracks, {
        signal: controller.signal,
        onProgress: (x) => progress.update(`${prefix}«${p.title}»: ${x.done} из ${x.total}, найдено ${x.found}`),
      });
      if (controller.signal.aborted) break;
      if (!tracks.length) continue;
      if (p.kind === "liked") {
        const lib = useLibrary.getState();
        for (const t of tracks) if (!lib.likes.some((l) => l.track.id === t.id)) useLibrary.getState().toggleLike(t);
        toast(`В понравившиеся добавлено ${tracks.length} из ${p.tracks.length}`);
      } else {
        const lib = useLibrary.getState();
        const created = lib.createPlaylist(p.title, p.description);
        lib.addToPlaylist(created.id, tracks);
        lastId = created.id;
        toast(`«${p.title}»: ${tracks.length} из ${p.tracks.length}`);
      }
    }
    if (lastId && !controller.signal.aborted) useNavigation.getState().go("playlist", lastId);
    progress.finish(controller.signal.aborted ? { ok: false, text: "Загрузка отменена" } : { ok: true, text: "Готово · импорт закончен" });
  } catch (e) {
    progress.finish(controller.signal.aborted ? { ok: false, text: "Загрузка отменена" } : { ok: false, text: `Не загрузилось: ${e instanceof Error ? e.message : String(e)}` });
  }
}

export default function SpotifyExportDialog() {
  const open = useSpotifyDialog((s) => s.open);
  const [found, setFound] = useState<SpotifyExportPlaylist[] | null>(null);
  const [chosen, setChosen] = useState<Set<number>>(new Set());
  const [error, setError] = useState("");
  const close = () => {
    useSpotifyDialog.setState({ open: false });
    setFound(null);
    setError("");
  };
  return (
    <Dialog title="Импорт из Spotify" open={open} onClose={close} wide>
      <div className="dialog-form">
        <p className="muted small">
          На spotify.com → «Конфиденциальность» → «Скачать свои данные» запросите архив (приходит на почту за несколько дней). Выберите здесь
          весь zip или JSON-файлы из него: YourLibrary.json, Playlist*.json, StreamingHistory*.json. EvPlayer найдёт эти песни на YouTube
          Music и SoundCloud.
        </p>
        <label className="btn ghost file-pick">
          Выбрать zip или JSON
          <input
            type="file"
            accept=".zip,.json"
            multiple
            hidden
            onChange={async (e) => {
              const files = [...(e.target.files ?? [])];
              e.target.value = "";
              if (!files.length) return;
              try {
                const lists = parseSpotifyExport(await readExportFiles(files), LABELS);
                if (!lists.length) throw new Error("empty");
                setFound(lists);
                setChosen(new Set(lists.map((_, i) => i)));
                setError("");
              } catch {
                setFound(null);
                setError("В файлах не нашлось плейлистов или понравившихся. Нужен архив «Данные аккаунта» от Spotify.");
              }
            }}
          />
        </label>
        {error && <p className="field-error">{error}</p>}
        {found && (
          <div className="check-list">
            {found.map((p, i) => (
              <label key={i} className="check-row">
                <input
                  type="checkbox"
                  checked={chosen.has(i)}
                  onChange={(e) => {
                    const next = new Set(chosen);
                    if (e.target.checked) next.add(i);
                    else next.delete(i);
                    setChosen(next);
                  }}
                />
                <span className="check-title">{p.title}</span>
                <span className="muted small">{p.tracks.length} треков</span>
              </label>
            ))}
          </div>
        )}
        <div className="dialog-actions">
          <button type="button" className="btn ghost" onClick={close}>Отмена</button>
          <button
            type="button"
            className="btn primary"
            disabled={!found || chosen.size === 0}
            onClick={() => {
              const list = found!.filter((_, i) => chosen.has(i));
              close();
              void importPlaylists(list);
            }}
          >
            Импортировать
          </button>
        </div>
      </div>
    </Dialog>
  );
}
