import { useEffect, useState } from "react";
import Dialog from "@/ui/components/Dialog";
import { toast } from "@/ui/toasts";
import { useLibrary } from "./store";
import { useLibraryDialogs } from "./dialogs";
import { decodePlaylistCode } from "./playlistCode";
import { goToPlaylist } from "./trackMenu";
import { importFromLink, linkKind } from "./importLink";

function CreateDialog() {
  const state = useLibraryDialogs((s) => s.create);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  useEffect(() => {
    if (state) {
      setTitle("");
      setDescription("");
    }
  }, [state]);
  const close = () => useLibraryDialogs.setState({ create: null });
  return (
    <Dialog title="Новый плейлист" open={Boolean(state)} onClose={close}>
      <form
        className="dialog-form"
        onSubmit={(e) => {
          e.preventDefault();
          const lib = useLibrary.getState();
          const playlist = lib.createPlaylist(title || "Новый плейлист", description);
          if (state?.tracks.length) lib.addToPlaylist(playlist.id, state.tracks);
          close();
          toast(`Плейлист «${playlist.title}» создан`);
          if (!state?.tracks.length) goToPlaylist(playlist.id);
        }}
      >
        <label className="field">
          <span>Название</span>
          <input id="pl-title" autoFocus value={title} maxLength={60} onChange={(e) => setTitle(e.target.value)} placeholder="Например, «Ночная поездка»" />
        </label>
        <label className="field">
          <span>Описание (необязательно)</span>
          <textarea id="pl-desc" value={description} maxLength={300} rows={2} onChange={(e) => setDescription(e.target.value)} />
        </label>
        <div className="dialog-actions">
          <button type="button" className="btn ghost" onClick={close}>Отмена</button>
          <button type="submit" className="btn primary">Создать</button>
        </div>
      </form>
    </Dialog>
  );
}

function ImportDialog() {
  const target = useLibraryDialogs((s) => s.importCode);
  const open = target !== false;
  const intoLikes = target === "likes";
  const [code, setCode] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (open) {
      setCode("");
      setError("");
    }
  }, [open]);
  const close = () => useLibraryDialogs.setState({ importCode: false });
  return (
    <Dialog title={intoLikes ? "Добавить в понравившиеся" : "Импорт плейлиста"} open={open} onClose={close}>
      <form
        className="dialog-form"
        onSubmit={async (e) => {
          e.preventDefault();
          const value = code.trim();
          const kind = linkKind(value);
          if (!kind) {
            setError("Это не похоже ни на ссылку, ни на код EvPlayer (он начинается с EVP).");
            return;
          }
          if (kind === "code") {
            // a broken code is told here, before the dialog closes
            setBusy(true);
            try {
              await decodePlaylistCode(value);
            } catch {
              setError("Код повреждён или неполный. Скопируйте его целиком, начиная с EVP.");
              return;
            } finally {
              setBusy(false);
            }
          }
          close();
          // the progress panel at the top shows how it goes
          void importFromLink(value, intoLikes ? "likes" : undefined);
        }}
      >
        <p className="muted small">
          {intoLikes
            ? "Вставьте код EvPlayer (строка с EVP) — например, скопированный из чьих-то «Понравившихся», — или ссылку на плейлист. Треки добавятся к вашим лайкам, повторы пропускаются."
            : "Вставьте ссылку на плейлист, альбом или трек из Spotify, Яндекс Музыки, YouTube / YouTube Music, SoundCloud или Deezer — или код EvPlayer (строка с EVP), который прислал друг. Код из «Понравившихся» добавится к вашим лайкам."}
        </p>
        <label className="field">
          <span>Ссылка или код</span>
          <textarea id="pl-code" autoFocus rows={4} value={code} onChange={(e) => { setCode(e.target.value); setError(""); }} placeholder="https://open.spotify.com/playlist/… или EVP2.…" spellCheck={false} />
        </label>
        {error && <p className="field-error">{error}</p>}
        <div className="dialog-actions">
          <button type="button" className="btn ghost" onClick={close}>Отмена</button>
          <button type="submit" className="btn primary" disabled={busy || !code.trim()}>{intoLikes ? "Добавить" : "Импортировать"}</button>
        </div>
      </form>
    </Dialog>
  );
}

export default function LibraryDialogs() {
  return (
    <>
      <CreateDialog />
      <ImportDialog />
    </>
  );
}
