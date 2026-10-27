import { useState } from "react";
import { useSettings } from "@/settings/store";
import { connectSoundCloud, disconnectSoundCloud, importSoundCloudLikes, importSoundCloudPlaylists } from "@/services/soundcloud/account";
import { importFromLink, linkKind } from "@/library/importLink";
import { openSpotifyExport } from "@/library/SpotifyExportDialog";
import { useImportProgress } from "@/library/importProgress";
import { useNavigation } from "@/ui/navigation";
import { importSpotifyLiked, importSpotifyPlaylists, signInSpotify, signOutSpotify } from "@/services/spotify/account";
import { DEFAULT_SPOTIFY_CLIENT_ID } from "@/settings/store";
import { connectYandex, disconnectYandex, importYandexLikes, importYandexPlaylists } from "@/services/yandex/account";
import { useLocalFiles } from "@/files/localFiles";
import { formatCount } from "@/ui/format";
import { toast } from "@/ui/toasts";
import { formatAddedAt } from "@/ui/dates";
import { IconFolder, IconHeart, IconLink, IconListPlus, IconPlug, IconRefresh, IconUser } from "@/ui/components/Icons";
import { Card, Pill, SwitchRow, TabHeader } from "./parts";

function ServiceBadge({ name, color }: { name: string; color: string }) {
  return (
    <span className="svc-badge" style={{ "--c": color } as React.CSSProperties} aria-hidden="true">
      {name.slice(0, 1)}
    </span>
  );
}

function ImportBox() {
  const [value, setValue] = useState("");
  const busy = useImportProgress((s) => s.text !== null);
  const kind = linkKind(value);
  const names = { soundcloud: "SoundCloud", youtube: "YouTube", spotify: "Spotify", deezer: "Deezer", yandex: "Яндекс Музыка", code: "код EvPlayer", apple: "Apple Music" } as const;
  return (
    <form
      className="import-box"
      onSubmit={(e) => {
        e.preventDefault();
        if (!kind) return toast("Это не похоже на ссылку на плейлист, альбом или трек", "error");
        void importFromLink(value.trim());
        setValue("");
      }}
    >
      <span className="import-box-icon"><IconLink size={18} /></span>
      <input value={value} onChange={(e) => setValue(e.target.value)} placeholder="Ссылка из Spotify, Яндекс Музыки, YouTube, SoundCloud, Deezer или код EVP…" spellCheck={false} />
      {kind && <Pill tone="info">{names[kind]}</Pill>}
      <button type="submit" className="btn primary" disabled={!value.trim() || busy}>Импортировать</button>
    </form>
  );
}

function SoundCloudCard() {
  const account = useSettings((s) => s.soundcloudAccount);
  const ownId = useSettings((s) => s.soundcloudClientId);
  const set = useSettings((s) => s.set);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState<"" | "connect" | "likes" | "playlists">("");
  const [advanced, setAdvanced] = useState(false);
  const go = useNavigation((s) => s.go);

  const run = async (what: typeof busy, job: () => Promise<void>) => {
    setBusy(what);
    try {
      await job();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Не получилось", "error");
    } finally {
      setBusy("");
    }
  };

  return (
    <section className="svc-card">
      <div className="svc-head">
        <ServiceBadge name="SoundCloud" color="#ff7a1a" />
        <span className="svc-head-text">
          <span className="svc-name">SoundCloud</span>
          <span className="muted small">Поиск, чарты, исполнители и полноценное воспроизведение</span>
        </span>
        <Pill tone={account ? "ok" : "info"}>{account ? "аккаунт подключён" : "работает без входа"}</Pill>
      </div>

      {account ? (
        <>
          <div className="svc-account">
            <span className="svc-avatar">{account.avatar ? <img src={account.avatar} alt="" /> : <IconUser size={22} />}</span>
            <span className="svc-account-text">
              <button type="button" className="link-btn strong" onClick={() => go("artist", `scu:${account.id.replace(/^scu?:/, "")}`, account.name)}>{account.name}</button>
              <span className="muted small">
                {[account.followers !== undefined && `${formatCount(account.followers)} подписчиков`, account.likes !== undefined && `${formatCount(account.likes)} лайков`, account.lastSync && `синхронизировано ${formatAddedAt(account.lastSync)}`].filter(Boolean).join(" · ")}
              </span>
            </span>
            <button type="button" className="btn ghost" onClick={disconnectSoundCloud}>Отключить</button>
          </div>
          <div className="svc-actions">
            <button
              type="button"
              className="btn ghost"
              disabled={busy !== ""}
              onClick={() => run("likes", async () => {
                const n = await importSoundCloudLikes();
                toast(n ? `В понравившиеся добавлено ${n}` : "Новых лайков нет");
              })}
            >
              {busy === "likes" ? <span className="spinner small-spin" /> : <IconHeart size={15} />} Забрать лайки
            </button>
            <button
              type="button"
              className="btn ghost"
              disabled={busy !== ""}
              onClick={() => run("playlists", async () => {
                const r = await importSoundCloudPlaylists();
                toast(r.created || r.updated ? `Плейлистов: новых ${r.created}, обновлено ${r.updated}` : "Публичных плейлистов не нашлось");
              })}
            >
              {busy === "playlists" ? <span className="spinner small-spin" /> : <IconListPlus size={15} />} Забрать плейлисты
            </button>
          </div>
          <SwitchRow title="Синхронизировать лайки" hint="При запуске новые лайки с SoundCloud сами появятся в «Понравившихся»" checked={account.syncLikes} onChange={(v) => set({ soundcloudAccount: { ...account, syncLikes: v } })} icon={<IconRefresh size={16} />} />
        </>
      ) : (
        <form
          className="svc-connect"
          onSubmit={(e) => {
            e.preventDefault();
            if (input.trim()) void run("connect", async () => {
              await connectSoundCloud(input);
              toast("Аккаунт SoundCloud подключён");
              const n = await importSoundCloudLikes();
              if (n) toast(`В понравившиеся добавлено ${n}`);
            });
          }}
        >
          <input value={input} onChange={(e) => setInput(e.target.value)} placeholder="Ссылка на профиль или имя: soundcloud.com/ваше-имя" spellCheck={false} />
          <button type="submit" className="btn primary" disabled={!input.trim() || busy !== ""}>
            {busy === "connect" ? <span className="spinner small-spin" /> : <IconPlug size={15} />} Подключить
          </button>
        </form>
      )}
      <p className="muted small">Подтягиваются ваши публичные лайки и плейлисты. Пароль не нужен и никуда не передаётся.</p>

      <button type="button" className="svc-more" onClick={() => setAdvanced((v) => !v)}>{advanced ? "Скрыть дополнительно" : "Дополнительно"}</button>
      {advanced && (
        <label className="field">
          <span>client_id своего приложения SoundCloud</span>
          <input value={ownId} onChange={(e) => set({ soundcloudClientId: e.target.value.trim() })} placeholder="Пусто — определяется автоматически" spellCheck={false} />
          <span className="toggle-hint">Если у вас есть приложение на soundcloud.com/you/apps, EvPlayer будет ходить в SoundCloud от его имени.</span>
        </label>
      )}
    </section>
  );
}

function YouTubeCard() {
  const match = useSettings((s) => s.youtubeMatchSoundCloud);
  const set = useSettings((s) => s.set);
  return (
    <section className="svc-card">
      <div className="svc-head">
        <ServiceBadge name="YouTube Music" color="#ff3b3b" />
        <span className="svc-head-text">
          <span className="svc-name">YouTube Music</span>
          <span className="muted small">Поиск песен, альбомов, плейлистов и страницы исполнителей</span>
        </span>
        <Pill tone="info">работает без входа</Pill>
      </div>
      <SwitchRow
        title="Играть через копию на SoundCloud"
        hint="Тогда работают эквалайзер, скорость, тон и реверберация. Если копии нет — играет официальный плеер YouTube в углу окна"
        checked={match}
        onChange={(v) => set({ youtubeMatchSoundCloud: v })}
      />
      <p className="muted small">Плейлисты YouTube и YouTube Music можно перенести по ссылке — поле выше.</p>
    </section>
  );
}

function SpotifyCard() {
  const id = useSettings((s) => s.spotifyClientId);
  const secret = useSettings((s) => s.spotifyClientSecret);
  const account = useSettings((s) => s.spotifyAccount);
  const set = useSettings((s) => s.set);
  const busy = useImportProgress((s) => s.text !== null);
  const [open, setOpen] = useState(false);
  const [working, setWorking] = useState(false);
  const builtIn = Boolean(DEFAULT_SPOTIFY_CLIENT_ID);
  const canSignIn = Boolean(id.trim() || builtIn);
  const appOnly = !account && Boolean(id.trim() && secret.trim());
  return (
    <section className="svc-card">
      <div className="svc-head">
        <ServiceBadge name="Spotify" color="#1ed760" />
        <span className="svc-head-text">
          <span className="svc-name">Spotify</span>
          <span className="muted small">Перенос «Любимых треков» и плейлистов любой длины — песни находятся на YouTube Music и SoundCloud</span>
        </span>
        <Pill tone={account || appOnly ? "ok" : "info"}>{account ? "аккаунт подключён" : appOnly ? "приложение подключено" : "до 100 треков"}</Pill>
      </div>

      {account ? (
        <>
          <div className="svc-account">
            <span className="svc-avatar">{account.avatar ? <img src={account.avatar} alt="" /> : <IconUser size={22} />}</span>
            <span className="svc-account-text">
              <span className="set-row-title">{account.name}</span>
              <span className="muted small">{account.product === "premium" ? "Spotify Premium" : "Spotify"} · плейлисты по ссылке читаются целиком</span>
            </span>
            <button type="button" className="btn ghost" onClick={signOutSpotify}>Выйти</button>
          </div>
          <div className="svc-actions">
            <button type="button" className="btn ghost" disabled={busy} onClick={() => void importSpotifyLiked()}><IconHeart size={15} /> Забрать «Любимые треки»</button>
            <button type="button" className="btn ghost" disabled={busy} onClick={() => void importSpotifyPlaylists()}><IconListPlus size={15} /> Забрать плейлисты</button>
          </div>
        </>
      ) : (
        <>
          <p className="muted small">
            Войдите своим аккаунтом Spotify (Premium не нужен) — и импорт заберёт плейлисты любой длины, а ещё ваши «Любимые треки». Вход открывается в
            браузере на spotify.com, пароль EvPlayer не видит.
          </p>
          <div className="svc-actions">
            <button
              type="button"
              className="btn primary"
              disabled={!canSignIn || working}
              onClick={async () => {
                setWorking(true);
                try {
                  await signInSpotify();
                  toast("Spotify подключён");
                } catch (e) {
                  toast(e instanceof Error ? e.message : "Не получилось", "error");
                } finally {
                  setWorking(false);
                }
              }}
            >
              {working ? <span className="spinner small-spin" /> : <IconPlug size={15} />} {working ? "Жду вход в браузере…" : "Войти через Spotify"}
            </button>
            <button type="button" className="btn ghost" onClick={openSpotifyExport}>Импорт архива данных</button>
          </div>
          {!canSignIn && <p className="muted small">Для входа нужен Client ID приложения Spotify — откройте «Своё приложение Spotify» ниже.</p>}
        </>
      )}

      <button type="button" className="svc-more" onClick={() => setOpen((v) => !v)}>{open ? "Скрыть" : "Своё приложение Spotify"}</button>
      {open && (
        <>
          <ol className="steps">
            <li>Откройте <button type="button" className="link-btn strong" onClick={() => window.open("https://developer.spotify.com/dashboard", "_blank")}>developer.spotify.com/dashboard</button> и войдите аккаунтом Spotify.</li>
            <li>«Create app»: любое имя, Redirect URI — <code>http://127.0.0.1:43821/callback</code>, отметьте «Web API».</li>
            <li>Скопируйте <b>Client ID</b> сюда. Пока приложение в режиме разработки, войти в него могут только аккаунты, добавленные в «User Management» (до 25).</li>
          </ol>
          <label className="field">
            <span>Client ID{builtIn ? " (пусто — встроенный EvPlayer)" : ""}</span>
            <input value={id} onChange={(e) => set({ spotifyClientId: e.target.value.trim() })} spellCheck={false} />
          </label>
          <label className="field">
            <span>Client Secret — необязательно</span>
            <input type="password" value={secret} onChange={(e) => set({ spotifyClientSecret: e.target.value.trim() })} spellCheck={false} />
            <span className="toggle-hint">Только если не хотите входить: тогда по ссылке читаются публичные плейлисты, но без «Любимых треков». Хранится только на этом компьютере.</span>
          </label>
        </>
      )}
    </section>
  );
}

function YandexCard() {
  const account = useSettings((s) => s.yandexAccount);
  const busy = useImportProgress((s) => s.text !== null);
  const [input, setInput] = useState("");
  const [connecting, setConnecting] = useState(false);
  return (
    <section className="svc-card">
      <div className="svc-head">
        <ServiceBadge name="Яндекс" color="#ffcc00" />
        <span className="svc-head-text">
          <span className="svc-name">Яндекс Музыка</span>
          <span className="muted small">Перенос «Мне нравится», плейлистов и альбомов — песни находятся на YouTube Music и SoundCloud</span>
        </span>
        <Pill tone={account ? "ok" : "info"}>{account ? "профиль подключён" : "перенос"}</Pill>
      </div>
      {account ? (
        <>
          <div className="svc-account">
            <span className="svc-avatar ym-avatar">{account.name.slice(0, 1).toUpperCase()}</span>
            <span className="svc-account-text">
              <span className="set-row-title">{account.name}</span>
              <span className="muted small">@{account.login} · публичных плейлистов: {account.playlists}</span>
            </span>
            <button type="button" className="btn ghost" onClick={disconnectYandex}>Отключить</button>
          </div>
          <div className="svc-actions">
            <button type="button" className="btn ghost" disabled={busy} onClick={() => void importYandexLikes()}><IconHeart size={15} /> Забрать «Мне нравится»</button>
            <button type="button" className="btn ghost" disabled={busy} onClick={() => void importYandexPlaylists()}><IconListPlus size={15} /> Забрать плейлисты</button>
          </div>
        </>
      ) : (
        <form
          className="svc-connect"
          onSubmit={async (e) => {
            e.preventDefault();
            if (!input.trim()) return;
            setConnecting(true);
            try {
              await connectYandex(input);
              toast("Профиль Яндекс Музыки подключён");
            } catch (err) {
              toast(err instanceof Error ? err.message : "Не получилось", "error");
            } finally {
              setConnecting(false);
            }
          }}
        >
          <input value={input} onChange={(e) => setInput(e.target.value)} placeholder="Логин или ссылка: music.yandex.ru/users/ваш-логин" spellCheck={false} />
          <button type="submit" className="btn primary" disabled={!input.trim() || connecting}>
            {connecting ? <span className="spinner small-spin" /> : <IconPlug size={15} />} Подключить
          </button>
        </form>
      )}
      <p className="muted small">
        Читаются публичные плейлисты и «Мне нравится» (в Яндекс Музыке: Настройки → Приватность → открыть). Пароль не нужен. Отдельный плейлист или
        альбом можно перенести по ссылке — поле вверху.
      </p>
    </section>
  );
}

function OtherCard() {
  const files = useLocalFiles((s) => s.records.length);
  const go = useNavigation((s) => s.go);
  return (
    <section className="svc-card">
      <div className="svc-head">
        <span className="svc-badge" style={{ "--c": "#8ab4ff" } as React.CSSProperties} aria-hidden="true"><IconFolder size={18} /></span>
        <span className="svc-head-text">
          <span className="svc-name">Свои файлы</span>
          <span className="muted small">{files ? `${files} треков с компьютера` : "MP3, FLAC, M4A, OGG, WAV — с папками, которые обновляются сами"}</span>
        </span>
        <button type="button" className="btn ghost" onClick={() => go("files")}>Открыть</button>
      </div>
      <div className="svc-head">
        <ServiceBadge name="Deezer" color="#a238ff" />
        <span className="svc-head-text">
          <span className="svc-name">Deezer</span>
          <span className="muted small">Плейлисты и альбомы переносятся по ссылке</span>
        </span>
        <Pill tone="info">перенос</Pill>
      </div>
      <div className="svc-head">
        <ServiceBadge name="Apple Music" color="#fa2d48" />
        <span className="svc-head-text">
          <span className="svc-name">Apple Music</span>
          <span className="muted small">Apple не открывает плейлисты сторонним программам. Перенесите их в Spotify (например, через Soundiiz) и импортируйте оттуда</span>
        </span>
        <Pill tone="off">недоступно</Pill>
      </div>
    </section>
  );
}

export default function ServicesTab() {
  return (
    <>
      <TabHeader icon={<IconPlug size={22} />} hue={25} title="Сервисы и импорт" description="Откуда EvPlayer берёт музыку и как перенести ваши плейлисты из других сервисов." />
      <Card title="Импорт по ссылке" hint="Плейлист, альбом или трек станет вашим плейлистом. Песни из Spotify и Deezer находятся автоматически">
        <ImportBox />
      </Card>
      <div className="svc-grid">
        <SoundCloudCard />
        <YouTubeCard />
        <SpotifyCard />
        <YandexCard />
        <OtherCard />
      </div>
    </>
  );
}
