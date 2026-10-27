import { useEffect, useState, type ReactNode } from "react";
import type { UpdateState } from "@/platform/api";
import { useSettings } from "@/settings/store";
import { saveBackupFile, readBackup, restoreBackup, makeBackup } from "@/settings/backup";
import { Segmented } from "@/ui/components/Controls";
import { toast } from "@/ui/toasts";
import { useHistory } from "@/stats/history";
import { clearListeningStats } from "@/stats/listeningStats";
import { clearLyricsCache } from "@/ui/useLyrics";
import {
  IconChat, IconDatabase, IconHeartFill, IconEq, IconInfo, IconKeyboard, IconPalette, IconPip, IconPlayCircle, IconPlug, IconRefresh,
} from "@/ui/components/Icons";
import EqualizerSection from "./settings/EqualizerSection";
import KeysTab from "./settings/KeysTab";
import DiscordTab from "./settings/DiscordTab";
import ServicesTab from "./settings/ServicesTab";
import OverlayTab from "./settings/OverlayTab";
import { Card, Row, SwitchRow, TabHeader } from "./settings/parts";
import Dialog from "@/ui/components/Dialog";

type Tab = "playback" | "sound" | "appearance" | "keys" | "overlay" | "services" | "discord" | "data" | "about";

const TABS: Array<{ id: Tab; label: string; icon: (p: { size?: number }) => ReactNode; group: string }> = [
  { id: "playback", label: "Воспроизведение", icon: IconPlayCircle, group: "Музыка" },
  { id: "sound", label: "Звук и эквалайзер", icon: IconEq, group: "Музыка" },
  { id: "services", label: "Сервисы и импорт", icon: IconPlug, group: "Музыка" },
  { id: "appearance", label: "Внешний вид", icon: IconPalette, group: "Приложение" },
  { id: "keys", label: "Клавиши", icon: IconKeyboard, group: "Приложение" },
  { id: "overlay", label: "Мини-плеер", icon: IconPip, group: "Приложение" },
  { id: "discord", label: "Discord", icon: IconChat, group: "Приложение" },
  { id: "data", label: "Данные и копия", icon: IconDatabase, group: "Прочее" },
  { id: "about", label: "О программе", icon: IconInfo, group: "Прочее" },
];

function Playback() {
  const s = useSettings();
  return (
    <>
      <TabHeader icon={<IconPlayCircle size={22} />} hue={160} title="Воспроизведение" description="Что происходит, когда музыка заканчивается, запускается или меняется устройство." />
      <Card title="Очередь">
        <SwitchRow title="Автовоспроизведение" hint="Когда очередь закончится, продолжить похожими треками" checked={s.autoplay} onChange={(v) => s.set({ autoplay: v })} />
        <Row title="Плавный переход" hint={s.crossfade ? `Следующий трек вступает за ${s.crossfade} с до конца текущего` : "Выключен: следующий трек готовится заранее и начинается сразу, без паузы"}>
          <input
            type="range"
            className="dial small-dial"
            min={0}
            max={12}
            step={1}
            value={s.crossfade}
            aria-label="Плавный переход, секунд"
            onChange={(e) => s.set({ crossfade: Number(e.target.value) })}
            style={{ "--fill": `${(s.crossfade / 12) * 100}%` } as React.CSSProperties}
          />
          <span className="muted small tabular">{s.crossfade ? `${s.crossfade} с` : "выкл"}</span>
        </Row>
        <SwitchRow title="Продолжать с места" hint="При запуске вернуть очередь и позицию — останется только нажать «Играть»" checked={s.resumeOnStart} onChange={(v) => s.set({ resumeOnStart: v })} />
        <SwitchRow title="Запуск двойным кликом" hint="Одиночный клик по треку только выделяет его — удобно, если часто промахиваетесь" checked={s.doubleClickToPlay} onChange={(v) => s.set({ doubleClickToPlay: v })} />
      </Card>
      <Card title="Устройства">
        <SwitchRow title="Пауза при отключении наушников" hint="Если пропадёт устройство вывода, музыка остановится, а не заиграет из колонок" checked={s.pauseOnDeviceChange} onChange={(v) => s.set({ pauseOnDeviceChange: v })} />
      </Card>
      <Card title="Поиск и тексты">
        <Row title="Где искать по умолчанию">
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
        </Row>
        <SwitchRow title="Текст с Genius" hint="Если в LRCLIB нет текста, взять обычный (без синхронизации) с genius.com" checked={s.geniusFallback} onChange={(v) => s.set({ geniusFallback: v })} />
      </Card>
      <Card title="Окно">
        <SwitchRow title="Закрытие прячет в трей" hint="Крестик убирает окно к часам, музыка продолжает играть. Выйти — через меню значка" checked={s.closeToTray} onChange={(v) => s.set({ closeToTray: v })} />
        <SwitchRow title="Сворачивать в трей" hint="Свёрнутое окно не занимает место на панели задач" checked={s.minimizeToTray} onChange={(v) => s.set({ minimizeToTray: v })} />
      </Card>
    </>
  );
}

function Sound() {
  return (
    <>
      <TabHeader icon={<IconEq size={22} />} hue={140} title="Звук и эквалайзер" description="Подстройте звучание под наушники и вкус. Скорость, тон и реверберация — в кнопке «Звук» у плеера." />
      <Card>
        <EqualizerSection />
      </Card>
    </>
  );
}

const DONT_KEEP = [
  { key: "noCache", title: "Обложки и картинки", hint: "Не копятся на диске — скачиваются заново. Включается полностью после перезапуска" },
  { key: "noServiceCache", title: "Ответы сервисов", hint: "Рекомендации, похожие треки и тексты песен ищутся каждый раз заново" },
  { key: "noLoudnessMemory", title: "Громкость треков", hint: "«Выравнивать громкость» не запоминает, насколько громкий каждый трек" },
  { key: "noStats", title: "Статистика", hint: "Топы, графики и умные плейлисты перестают пополняться" },
  { key: "noHistory", title: "История", hint: "«Недавно слушали» на главной больше не пополняется" },
] as const;

/** what EvPlayer should not remember; each ticked item frees disk or memory */
function DontKeepList() {
  const s = useSettings();
  return (
    <div className="dont-keep">
      <span className="eyebrow">Не сохранять</span>
      {DONT_KEEP.map((item) => (
        <label key={item.key} className="check-row">
          <input
            type="checkbox"
            checked={s[item.key]}
            onChange={(e) => {
              s.set({ [item.key]: e.target.checked });
              if (e.target.checked && item.key === "noServiceCache") clearLyricsCache();
            }}
          />
          <span className="check-text">
            <span className="check-title">{item.title}</span>
            <span className="muted small">{item.hint}</span>
          </span>
        </label>
      ))}
    </div>
  );
}

function Appearance() {
  const s = useSettings();
  return (
    <>
      <TabHeader icon={<IconPalette size={22} />} hue={300} title="Внешний вид" description="Фон, свечение и текст песен." />
      <Card title="Экономия ресурсов">
        <SwitchRow
          title="Режим экономии памяти"
          hint="Без живого фона, свечения, размытия и анимаций; спрятанное окно освобождает память через 4 секунды. Поиск, плейлисты, музыка и мини-плеер работают как обычно"
          checked={s.memorySaver}
          onChange={(v) => s.set({ memorySaver: v })}
        />
        <DontKeepList />
      </Card>
      <Card title="Фон и обложка">
        <SwitchRow title="Живой фон из обложки" hint="Медленно переливающийся фон в цветах трека. Выключите на слабом компьютере" checked={s.animatedBackdrop} onChange={(v) => s.set({ animatedBackdrop: v })} />
        <SwitchRow title="Панель «Далее»" hint="Очередь справа на экране «Сейчас играет». Можно скрыть, если не хотите знать, что будет дальше" checked={s.showQueue} onChange={(v) => s.set({ showQueue: v })} />
        <SwitchRow title="Свечение обложки" hint="Мягкий ореол вокруг большой обложки на экране «Сейчас играет»" checked={s.coverGlow} onChange={(v) => s.set({ coverGlow: v })} />
      </Card>
      <Card title="Текст песни">
        <SwitchRow title="Размывать дальние строки" hint="Текущая строка резкая, остальные чуть размыты" checked={s.lyricsBlur} onChange={(v) => s.set({ lyricsBlur: v })} />
        <Row title="Размер текста">
          <Segmented
            label="Размер текста"
            value={s.lyricsSize}
            onChange={(v) => s.set({ lyricsSize: v })}
            options={[
              { value: "s", label: "Меньше" },
              { value: "m", label: "Обычный" },
              { value: "l", label: "Крупнее" },
            ]}
          />
        </Row>
      </Card>
      <p className="muted small">Анимации и фон сами останавливаются, когда окно свёрнуто или спрятано в трей.</p>
    </>
  );
}

function sizeOf(text: string): string {
  const kb = new Blob([text]).size / 1024;
  return kb > 1024 ? `${(kb / 1024).toFixed(1)} МБ` : `${Math.max(1, Math.round(kb))} КБ`;
}

function DataTab() {
  const clearHistory = useHistory((h) => h.clear);
  const [cache, setCache] = useState<number | null>(null);
  const [confirm, setConfirm] = useState<null | "stats" | "restore">(null);
  const [pending, setPending] = useState<ReturnType<typeof readBackup> | null>(null);
  const [usage] = useState(() => {
    const data = makeBackup().data;
    const pick = (re: RegExp) => Object.entries(data).filter(([k]) => re.test(k)).map(([, v]) => v).join("");
    return { library: sizeOf(pick(/library/)), stats: sizeOf(pick(/stats|history/)), all: sizeOf(Object.values(data).join("")) };
  });
  useEffect(() => {
    void window.evplayer?.cacheSize().then((r) => setCache(r.cache));
  }, []);
  return (
    <>
      <TabHeader icon={<IconDatabase size={22} />} hue={45} title="Данные и копия" description="Всё хранится только на этом компьютере. Сделайте копию, чтобы перенести библиотеку." />
      <div className="usage-grid">
        <div className="stat"><span className="stat-label">Библиотека</span><span className="stat-value small-value">{usage.library}</span></div>
        <div className="stat"><span className="stat-label">Статистика и история</span><span className="stat-value small-value">{usage.stats}</span></div>
        <div className="stat"><span className="stat-label">Кэш обложек и ответов</span><span className="stat-value small-value">{cache === null ? "—" : `${(cache / 1024 / 1024).toFixed(0)} МБ`}</span></div>
      </div>
      <Card title="Резервная копия" hint="Лайки, плейлисты, статистика, история, пресеты и настройки одним файлом">
        <div className="svc-actions">
          <button type="button" className="btn primary" onClick={() => void saveBackupFile().then((ok) => ok && toast("Копия сохранена"))}>Сохранить копию</button>
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
            Восстановить из копии
          </button>
        </div>
      </Card>
      <Card title="Очистка">
        <Row title="Кэш" hint="Обложки и ответы сервисов — скачаются заново, когда понадобятся">
          <button type="button" className="btn ghost" onClick={async () => { await window.evplayer?.clearCache(); clearLyricsCache(); setCache(0); toast("Кэш очищен"); }}>Очистить</button>
        </Row>
        <Row title="История прослушиваний" hint="Список «Недавно слушали» на главной">
          <button type="button" className="btn ghost" onClick={() => { clearHistory(); toast("История очищена"); }}>Очистить</button>
        </Row>
        <Row title="Статистика" hint="Топы, графики и умные плейлисты строятся из неё">
          <button type="button" className="btn ghost" onClick={() => setConfirm("stats")}>Сбросить</button>
        </Row>
        <Row title="Папка с данными" hint="Список ваших файлов и обложки к ним">
          <button type="button" className="btn ghost" onClick={() => void window.evplayer?.openDataFolder()}>Открыть</button>
        </Row>
      </Card>
      <Dialog title={confirm === "stats" ? "Сбросить статистику?" : "Восстановить из копии?"} open={confirm !== null} onClose={() => setConfirm(null)}>
        <p className="muted">
          {confirm === "stats"
            ? "Все прослушивания, топы и умные плейлисты обнулятся. Это нельзя отменить."
            : `Всё, что сейчас есть в EvPlayer на этом компьютере, заменится содержимым копии от ${pending ? new Date(pending.createdAt).toLocaleString("ru-RU") : ""}.`}
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
    </>
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
    : "Проверяются сами при запуске и каждые 4 часа";
  return (
    <>
      <div className="about-hero">
        <img src="./icon.png" alt="" className="about-icon" />
        <div>
          <h2 className="about-name">EvPlayer</h2>
          <p className="muted">Версия {version || "—"} · открытый проект, лицензия MIT</p>
        </div>
      </div>
      <div className="donate-card">
        <span className="donate-icon"><IconHeartFill size={22} /></span>
        <span className="donate-text">
          <span className="set-row-title">Поддержать автора</span>
          <span className="set-row-hint">EvPlayer бесплатный и без рекламы. Если он вам нравится, можно поддержать разработку через DonationAlerts.</span>
        </span>
        <button type="button" className="btn donate" onClick={() => window.open("https://www.donationalerts.com/r/evire", "_blank")}>Поддержать</button>
      </div>
      <Card>
        <Row title="Обновления" hint={status} icon={<IconRefresh size={16} />}>
          {update.status === "ready" ? (
            <button type="button" className="btn primary" onClick={() => void window.evplayer?.updates.install()}>Перезапустить</button>
          ) : (
            <button type="button" className="btn ghost" disabled={update.status === "checking" || update.status === "downloading"} onClick={() => void window.evplayer?.updates.check()}>Проверить</button>
          )}
        </Row>
        <Row title="Исходный код" hint="github.com/Evire1337/evplayer">
          <button type="button" className="btn ghost" onClick={() => window.open("https://github.com/Evire1337/evplayer", "_blank")}>Открыть</button>
        </Row>
      </Card>
      <p className="muted small">Музыка — SoundCloud и YouTube, тексты — LRCLIB и Genius. EvPlayer не связан с этими сервисами.</p>
    </>
  );
}

export default function SettingsPage() {
  const [tab, setTab] = useState<Tab>("playback");
  const groups = [...new Set(TABS.map((t) => t.group))];
  return (
    <div className="page settings-page">
      <header className="page-head">
        <h1 className="display">Настройки</h1>
      </header>
      <div className="settings-layout">
        <nav className="settings-tabs glass" aria-label="Разделы настроек">
          {groups.map((g) => (
            <div key={g} className="settings-tab-group">
              <span className="settings-tab-group-title">{g}</span>
              {TABS.filter((t) => t.group === g).map(({ id, label, icon: Icon }) => (
                <button key={id} type="button" className={tab === id ? "is-active" : ""} aria-current={tab === id ? "page" : undefined} onClick={() => setTab(id)}>
                  <Icon size={17} />
                  <span>{label}</span>
                </button>
              ))}
            </div>
          ))}
        </nav>
        <section className="settings-panel" key={tab}>
          {tab === "playback" && <Playback />}
          {tab === "sound" && <Sound />}
          {tab === "appearance" && <Appearance />}
          {tab === "keys" && <KeysTab />}
          {tab === "overlay" && <OverlayTab />}
          {tab === "services" && <ServicesTab />}
          {tab === "discord" && <DiscordTab />}
          {tab === "data" && <DataTab />}
          {tab === "about" && <About />}
        </section>
      </div>
    </div>
  );
}
