import { useEffect, useMemo, useState } from "react";
import Dialog from "@/ui/components/Dialog";
import { useLibrary } from "@/library/store";
import { toast } from "@/ui/toasts";
import { closeDownloadDialog, useDownloadDialog } from "./dialog";
import { keepTracks } from "./keep";
import { LIKES_SCOPE, scopeStatus, tracksOf } from "./scopes";
import { useKeepLists, useOffline } from "./store";

const songs = (n: number) => `${n} ${n % 10 === 1 && n % 100 !== 11 ? "трек" : n % 10 >= 2 && n % 10 <= 4 && (n % 100 < 12 || n % 100 > 14) ? "трека" : "треков"}`;

// "Save for offline": tick the likes, playlists or both and they are fetched together; the
// switch keeps them complete when songs are added later.
export default function DownloadDialog() {
  const { open, preset } = useDownloadDialog();
  const playlists = useLibrary((s) => s.playlists);
  const likes = useLibrary((s) => s.likes);
  const kept = useOffline((s) => s.entries);
  const keptLists = useKeepLists((s) => s.lists);
  const [picked, setPicked] = useState<string[]>([]);
  const [follow, setFollow] = useState(true);

  useEffect(() => {
    if (!open) return;
    setPicked(preset);
    setFollow(true);
  }, [open, preset]);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const rows = useMemo(() => [LIKES_SCOPE, ...playlists.map((p) => p.id)].map((scope) => ({ scope, ...scopeStatus(scope, kept) })), [playlists, likes, kept]);
  const titleOf = (scope: string) => (scope === LIKES_SCOPE ? "Понравившиеся" : (playlists.find((p) => p.id === scope)?.title ?? "Плейлист"));
  const listed = rows.filter((r) => r.total > 0);
  const missing = useMemo(() => tracksOf(picked).filter((t) => !(t.id in kept)).length, [picked, kept, likes, playlists]);
  const toggle = (scope: string) => setPicked((p) => (p.includes(scope) ? p.filter((s) => s !== scope) : [...p, scope]));
  const allOn = listed.length > 0 && listed.every((r) => picked.includes(r.scope));

  const start = () => {
    if (follow) useKeepLists.getState().add(picked);
    const added = keepTracks(tracksOf(picked));
    toast(added ? `Скачиваю для офлайн: ${songs(added)}` : "Всё выбранное уже сохранено");
    closeDownloadDialog();
  };

  return (
    <Dialog title="Скачать для офлайн" open={open} onClose={closeDownloadDialog}>
      <p className="muted">Отметьте, что сохранить на этом компьютере: лайки, плейлисты или всё сразу. Скачанное играет без интернета.</p>
      {listed.length === 0 ? (
        <p className="muted">Пока нечего скачивать: добавьте треки из SoundCloud или YouTube в лайки или плейлист.</p>
      ) : (
        <div className="check-list">
          <label className="check-row">
            <input type="checkbox" checked={allOn} onChange={() => setPicked(allOn ? [] : listed.map((r) => r.scope))} />
            <span className="check-title">Всё сразу</span>
          </label>
          {listed.map((r) => (
            <label key={r.scope} className="check-row">
              <input type="checkbox" checked={picked.includes(r.scope)} onChange={() => toggle(r.scope)} />
              <span className="check-title">{titleOf(r.scope)}</span>
              <span className="muted small">
                {r.kept === r.total ? "всё скачано · " : r.kept ? `скачано ${r.kept} из ` : ""}
                {songs(r.total)}
                {keptLists.includes(r.scope) ? " · следит" : ""}
              </span>
            </label>
          ))}
        </div>
      )}
      <label className="toggle-row">
        <span className="toggle-text">
          <span className="toggle-title">Докачивать новые треки</span>
          <span className="toggle-hint">Что добавите в эти списки потом, скачается само</span>
        </span>
        <input type="checkbox" role="switch" className="switch" aria-label="Докачивать новые треки" checked={follow} onChange={(e) => setFollow(e.target.checked)} />
      </label>
      <div className="dialog-actions">
        <button type="button" className="btn ghost" onClick={closeDownloadDialog}>Отмена</button>
        <button type="button" className="btn primary" disabled={picked.length === 0} onClick={start}>
          {picked.length === 0 ? "Скачать" : missing ? `Скачать ${songs(missing)}` : "Сохранить выбор"}
        </button>
      </div>
    </Dialog>
  );
}
