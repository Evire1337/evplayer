import { useLibrary } from "@/library/store";
import PlaylistCover from "@/library/Cover";
import { openCreatePlaylist, openImportCode } from "@/library/dialogs";
import { useNavigation } from "@/ui/navigation";
import { formatDate } from "@/ui/dates";
import { IconCode, IconPlus } from "@/ui/components/Icons";

function songs(n: number): string {
  const mod10 = n % 10;
  const mod100 = n % 100;
  const word = mod10 === 1 && mod100 !== 11 ? "трек" : mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14) ? "трека" : "треков";
  return `${n} ${word}`;
}

export { songs };

export default function LibraryPage() {
  const { likes, playlists } = useLibrary();
  const go = useNavigation((s) => s.go);
  return (
    <div className="page">
      <header className="page-head with-actions">
        <h1 className="display">Библиотека</h1>
        <div className="head-actions">
          <button type="button" className="btn primary" onClick={() => openCreatePlaylist()}>
            <IconPlus size={16} /> Новый плейлист
          </button>
          <button type="button" className="btn ghost" onClick={() => openImportCode()}>
            <IconCode size={16} /> Импорт по коду
          </button>
        </div>
      </header>

      <div className="pl-grid">
        <button type="button" className="pl-card" onClick={() => go("playlist", "likes")}>
          <PlaylistCover liked />
          <span className="pl-card-title">Понравившиеся</span>
          <span className="pl-card-sub">{songs(likes.length)}</span>
        </button>
        {playlists.map((p) => (
          <button key={p.id} type="button" className="pl-card" onClick={() => go("playlist", p.id)}>
            <PlaylistCover playlist={p} />
            <span className="pl-card-title">{p.title}</span>
            <span className="pl-card-sub">{songs(p.items.length)} · создан {formatDate(p.createdAt)}</span>
          </button>
        ))}
      </div>
      {playlists.length === 0 && (
        <p className="muted">Создайте свой первый плейлист: в него можно класть треки из SoundCloud, YouTube и ваших файлов вперемешку. Через правый клик по любому треку → «Добавить в плейлист».</p>
      )}
    </div>
  );
}
