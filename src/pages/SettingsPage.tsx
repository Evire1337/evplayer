import { useEffect, useState } from "react";
import type { UpdateState } from "../../electron/updates";
import { useSettings } from "@/settings/store";
import { saveBackupFile, readBackup, restoreBackup } from "@/settings/backup";
import { Segmented, SettingRow, Toggle } from "@/ui/components/Controls";
import { toast } from "@/ui/toasts";
import { useHistory } from "@/stats/history";
import { clearListeningStats } from "@/stats/listeningStats";
import { clearLyricsCache } from "@/ui/useLyrics";
import { openSpotifyExport } from "@/library/SpotifyExportDialog";
import { openImportCode } from "@/library/dialogs";
import { useNavigation } from "@/ui/navigation";
import EqualizerSection from "./settings/EqualizerSection";
import Dialog from "@/ui/components/Dialog";

type Tab = "playback" | "sound" | "appearance" | "window" | "services" | "discord" | "data" | "about";

const TABS: Array<{ id: Tab; label: string }> = [
  { id: "playback", label: "Воспроизведение" },
  { id: "sound", label: "Звук и эквалайзер" },
  { id: "appearance", label: "Внешний вид" },
  { id: "window", label: "Окно, трей и клавиши" },
  { id: "services", label: "Сервисы и импорт" },
  { id: "discord", label: "Discord" },
  { id: "data", label: "Данные и копия" },
  { id: "about", label: "О программе" },
];

const SHORTCUTS: Array<[string, string]> = [
  ["Пробел", "Играть / пауза"],
  ["Ctrl + →  /  Ctrl + ←", "Следующий / предыдущий трек"],
  ["→  /  ←", "Перемотка на 5 секунд"],
  ["↑  /  ↓", "Громкость"],
  ["M", "Без звука"],
  ["L", "Нравится"],
  ["S", "Перемешать"],
  ["R", "Повтор"],
  ["Ctrl + F", "Поиск"],
  ["Ctrl + ,", "Настройки"],
  ["Esc", "Закрыть окно / меню"],
];

const GLOBAL_SHORTCUTS: Array<[string, string]> = [
  ["Медиаклавиши клавиатуры", "Играть, следующий, предыдущий"],
  ["Ctrl + Alt + Пробел", "Играть / пауза из любой программы"],
  ["Ctrl + Alt + →  /  ←", "Следующий / предыдущий"],
];

function Playback() {
  const s = useSettings();
  return (
    <div className="settings-group">
      <Toggle id="autoplay" title="Автовоспроизведение" hint="Когда очередь закончится, продолжать похожими треками" checked={s.autoplay} onChange={(v) => s.set({ autoplay: v })} />
      <Toggle
        id="yt-sc"
        title="YouTube через SoundCloud"
        hint="Для песни с YouTube сначала искать ту же запись на SoundCloud: тогда работают эквалайзер, скорость, тон и реверберация. Если не нашлось — играет официальный плеер YouTube в углу окна."
        checked={s.youtubeMatchSoundCloud}
        onChange={(v) => s.set({ youtubeMatchSoundCloud: v })}
      />
      <SettingRow title="Где искать по умолчанию">
        <Segmented
          label="Где искать по умолчанию"
          value={s.defaultSearchSource}
          onChange={(v) => s.set({ defaultSearchSource: v })}
          options={[
            { value: "all", label: "Везде" },
            { value: "soundcloud", label: "SoundCloud" },
            { value: "youtube", label: "YouTube" },
          ]}
        />
      </SettingRow>
      <Toggle id="genius" title="Текст с Genius" hint="Если у трека нет текста в LRCLIB, взять обычный (без синхронизации) с genius.com" checked={s.geniusFallback} onChange={(v) => s.set({ geniusFallback: v })} />
      <p className="muted small">Скорость, тон и реверберация — в кнопке «Звук» рядом с плеером. Там же можно оставить их для следующих треков.</p>
    </div>
  );
}

function Appearance() {
  const s = useSettings();
  return (
    <div className="settings-group">
      <Toggle id="backdrop" title="Живой фон из обложки" hint="Медленно переливающийся фон в цветах текущего трека. Выключите, если компьютер слабый" checked={s.animatedBackdrop} onChange={(v) => s.set({ animatedBackdrop: v })} />
      <Toggle id="glow" title="Свечение обложки" hint="Мягкий ореол вокруг большой обложки на экране «Сейчас играет»" checked={s.coverGlow} onChange={(v) => s.set({ coverGlow: v })} />
      <Toggle id="lyrics-blur" title="Размывать дальние строки" hint="Текущая строка текста резкая, остальные чуть размыты" checked={s.lyricsBlur} onChange={(v) => s.set({ lyricsBlur: v })} />
      <SettingRow title="Размер текста песни">
        <Segmented
          label="Размер текста песни"
          value={s.lyricsSize}
          onChange={(v) => s.set({ lyricsSize: v })}
          options={[
            { value: "s", label: "Меньше" },
            { value: "m", label: "Обычный" },
            { value: "l", label: "Крупнее" },
          ]}
        />
      </SettingRow>
      <p className="muted small">Анимации и фон сами останавливаются, когда окно свёрнуто или спрятано в трей.</p>
    </div>
  );
}

function WindowTab() {
  const s = useSettings();
  return (
    <div className="settings-group">
      <Toggle id="close-tray" title="Закрытие прячет в трей" hint="Крестик убирает окно к часам, музыка продолжает играть. Выйти — через меню значка в трее" checked={s.closeToTray} onChange={(v) => s.set({ closeToTray: v })} />
      <Toggle id="min-tray" title="Сворачивать в трей" hint="Свёрнутое окно не занимает место на панели задач" checked={s.minimizeToTray} onChange={(v) => s.set({ minimizeToTray: v })} />
      <Toggle id="global-keys" title="Глобальные клавиши" hint="Управлять музыкой, даже когда EvPlayer не в фокусе" checked={s.globalShortcuts} onChange={(v) => s.set({ globalShortcuts: v })} />
      <h3 className="eyebrow">Клавиши в окне</h3>
      <dl className="keys">
        {SHORTCUTS.map(([k, v]) => (
          <div key={k}><dt><kbd>{k}</kbd></dt><dd>{v}</dd></div>
        ))}
      </dl>
      <h3 className="eyebrow">Глобальные</h3>
      <dl className="keys">
        {GLOBAL_SHORTCUTS.map(([k, v]) => (
          <div key={k}><dt><kbd>{k}</kbd></dt><dd>{v}</dd></div>
        ))}
      </dl>
    </div>
  );
}

function Services() {
  const s = useSettings();
  const go = useNavigation((n) => n.go);
  return (
    <div className="settings-group">
      <SettingRow title="Импорт по ссылке" hint="Плейлист, альбом или трек из Spotify, YouTube / YouTube Music, SoundCloud или Deezer, а также код EvPlayer от друга">
        <button type="button" className="btn ghost" onClick={openImportCode}>Вставить ссылку</button>
      </SettingRow>
      <SettingRow title="Данные Spotify" hint="Архив «Скачать свои данные» из аккаунта Spotify: любимые, плейлисты и самое слушаемое">
        <button type="button" className="btn ghost" onClick={openSpotifyExport}>Выбрать архив</button>
      </SettingRow>
      <SettingRow title="Свои файлы" hint="MP3, FLAC, M4A, OGG, WAV с компьютера; можно следить за папкой">
        <button type="button" className="btn ghost" onClick={() => go("files")}>Открыть</button>
      </SettingRow>
      <label className="field">
        <span>SoundCloud client_id своего приложения (необязательно)</span>
        <input value={s.soundcloudClientId} onChange={(e) => s.set({ soundcloudClientId: e.target.value.trim() })} placeholder="Пусто — определяется автоматически" spellCheck={false} />
        <span className="toggle-hint">Если вы зарегистрировали приложение на soundcloud.com/you/apps, вставьте его client_id — EvPlayer будет ходить в SoundCloud от его имени.</span>
      </label>
      <p className="muted small">
        Spotify и Apple Music не дают сторонним программам воспроизводить свою музыку, поэтому их плейлисты переносятся: каждая песня находится на YouTube Music или SoundCloud.
      </p>
    </div>
  );
}

function DiscordTab() {
  const s = useSettings();
  const [status, setStatus] = useState<string>("off");
  useEffect(() => {
    const tick = () => void window.evplayer?.discord.status().then(setStatus);
    tick();
    const timer = setInterval(tick, 3000);
    return () => clearInterval(timer);
  }, []);
  const label = { off: "выключено", connecting: "подключаюсь…", connected: "подключено", "no-discord": "Discord не запущен" }[status] ?? status;
  return (
    <div className="settings-group">
      <Toggle id="discord" title="«Слушает» в профиле Discord" hint="Показывать текущий трек в статусе Discord (нужен запущенный Discord для компьютера)" checked={s.discordEnabled} onChange={(v) => s.set({ discordEnabled: v })} />
      <label className="field">
        <span>Application ID</span>
        <input value={s.discordAppId} onChange={(e) => s.set({ discordAppId: e.target.value.replace(/\D/g, "") })} placeholder="например 1234567890123456789" spellCheck={false} />
        <span className="toggle-hint">
          Создайте приложение на discord.com/developers/applications (бесплатно), назовите его «EvPlayer» — это имя Discord покажет в статусе — и вставьте сюда его Application ID.
        </span>
      </label>
      <p className="muted small">Состояние: {label}</p>
    </div>
  );
}

function DataTab() {
  const clearHistory = useHistory((h) => h.clear);
  const [cache, setCache] = useState<number | null>(null);
  const [confirm, setConfirm] = useState<null | "stats" | "restore">(null);
  const [pending, setPending] = useState<ReturnType<typeof readBackup> | null>(null);
  useEffect(() => {
    void window.evplayer?.cacheSize().then((r) => setCache(r.cache));
  }, []);
  return (
    <div className="settings-group">
      <SettingRow title="Резервная копия" hint="Лайки, плейлисты, статистика, история, пресеты и настройки — одним файлом. Его можно перенести на другой компьютер">
        <button type="button" className="btn ghost" onClick={() => void saveBackupFile().then((ok) => ok && toast("Копия сохранена"))}>Сохранить</button>
        <button
          type="button"
          className="btn ghost"
          onClick={async () => {
            const files = (await window.evplayer?.files.openText(["json"])) ?? [];
            if (!files[0]) return;
            try {
              setPending(readBackup(files[0].text));
              setConfirm("restore");
            } catch (e) {
              toast(e instanceof Error ? e.message : "Не тот файл", "error");
            }
          }}
        >
          Восстановить
        </button>
      </SettingRow>
      <SettingRow title="Кэш" hint={cache === null ? "Обложки и ответы сервисов, которые можно скачать заново" : `Сейчас занято ${(cache / 1024 / 1024).toFixed(0)} МБ`}>
        <button
          type="button"
          className="btn ghost"
          onClick={async () => {
            await window.evplayer?.clearCache();
            clearLyricsCache();
            setCache(0);
            toast("Кэш очищен");
          }}
        >
          Очистить
        </button>
      </SettingRow>
      <SettingRow title="История прослушиваний" hint="Список «Недавно слушали» на главной">
        <button type="button" className="btn ghost" onClick={() => { clearHistory(); toast("История очищена"); }}>Очистить</button>
      </SettingRow>
      <SettingRow title="Статистика" hint="Топы, графики и умные плейлисты строятся из неё">
        <button type="button" className="btn ghost" onClick={() => setConfirm("stats")}>Сбросить</button>
      </SettingRow>
      <SettingRow title="Папка с данными" hint="Где EvPlayer хранит список ваших файлов и обложки к ним">
        <button type="button" className="btn ghost" onClick={() => void window.evplayer?.openDataFolder()}>Открыть</button>
      </SettingRow>
      <Dialog title={confirm === "stats" ? "Сбросить статистику?" : "Восстановить из копии?"} open={confirm !== null} onClose={() => setConfirm(null)}>
        <p className="muted">
          {confirm === "stats"
            ? "Все прослушивания, топы и умные плейлисты обнулятся. Это нельзя отменить."
            : `Всё, что сейчас есть в EvPlayer на этом компьютере (лайки, плейлисты, статистика, настройки), заменится содержимым копии от ${pending ? new Date(pending.createdAt).toLocaleString("ru-RU") : ""}.`}
        </p>
        <div className="dialog-actions">
          <button type="button" className="btn ghost" onClick={() => setConfirm(null)}>Отмена</button>
          <button
            type="button"
            className="btn danger"
            onClick={() => {
              if (confirm === "stats") {
                clearListeningStats();
                toast("Статистика сброшена");
                setConfirm(null);
              } else if (pending) {
                restoreBackup(pending);
                window.location.reload();
              }
            }}
          >
            {confirm === "stats" ? "Сбросить" : "Восстановить"}
          </button>
        </div>
      </Dialog>
    </div>
  );
}

function About() {
  const [version, setVersion] = useState("");
  const [update, setUpdate] = useState<UpdateState>({ status: "idle" });
  useEffect(() => {
    void window.evplayer?.version().then(setVersion);
    const bridge = window.evplayer?.updates;
    if (!bridge) return;
    void bridge.get().then(setUpdate);
    return bridge.onState(setUpdate);
  }, []);
  const status =
    update.status === "checking" ? "Проверяю…"
    : update.status === "downloading" ? `Скачиваю ${update.version}: ${Math.round(update.percent)}%`
    : update.status === "ready" ? `Версия ${update.version} готова — установится при перезапуске`
    : update.status === "latest" ? "У вас последняя версия"
    : update.status === "error" ? `Не получилось проверить: ${update.message}`
    : "Обновления проверяются сами при запуске и каждые 4 часа";
  return (
    <div className="settings-group">
      <div className="about">
        <img src="./icon.png" alt="" className="about-icon" />
        <div>
          <h2 className="about-name">EvPlayer</h2>
          <p className="muted">Версия {version || "—"}</p>
        </div>
      </div>
      <SettingRow title="Обновления" hint={status}>
        {update.status === "ready" ? (
          <button type="button" className="btn primary" onClick={() => void window.evplayer?.updates.install()}>Перезапустить</button>
        ) : (
          <button type="button" className="btn ghost" disabled={update.status === "checking" || update.status === "downloading"} onClick={() => void window.evplayer?.updates.check()}>Проверить</button>
        )}
      </SettingRow>
      <SettingRow title="Исходный код" hint="Открытый проект под лицензией MIT">
        <button type="button" className="btn ghost" onClick={() => window.open("https://github.com/Evire1337/evplayer", "_blank")}>GitHub</button>
      </SettingRow>
      <p className="muted small">
        Музыка играет с SoundCloud и YouTube через их публичные сервисы; тексты — LRCLIB и Genius. EvPlayer не связан с этими сервисами.
      </p>
    </div>
  );
}

export default function SettingsPage() {
  const [tab, setTab] = useState<Tab>("playback");
  return (
    <div className="page settings-page">
      <header className="page-head">
        <h1 className="display">Настройки</h1>
      </header>
      <div className="settings-layout">
        <nav className="settings-tabs glass" aria-label="Разделы настроек">
          {TABS.map((t) => (
            <button key={t.id} type="button" className={tab === t.id ? "is-active" : ""} aria-current={tab === t.id ? "page" : undefined} onClick={() => setTab(t.id)}>
              {t.label}
            </button>
          ))}
        </nav>
        <section className="settings-panel glass">
          <h2 className="settings-title">{TABS.find((t) => t.id === tab)?.label}</h2>
          {tab === "playback" && <Playback />}
          {tab === "sound" && <EqualizerSection />}
          {tab === "appearance" && <Appearance />}
          {tab === "window" && <WindowTab />}
          {tab === "services" && <Services />}
          {tab === "discord" && <DiscordTab />}
          {tab === "data" && <DataTab />}
          {tab === "about" && <About />}
        </section>
      </div>
    </div>
  );
}
