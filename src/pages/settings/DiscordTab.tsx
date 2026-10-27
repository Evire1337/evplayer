import { useEffect, useState } from "react";
import { DEFAULT_DISCORD_APP_ID, discordAppId, useSettings, type DiscordStatusShows } from "@/settings/store";
import { currentTrack, usePlayer } from "@/player/store";
import { artistLine, formatTime } from "@/ui/format";
import { Segmented } from "@/ui/components/Controls";
import { IconChat, IconMusic, IconUser } from "@/ui/components/Icons";
import type { DiscordUser } from "../../../electron/discord";
import { Card, Pill, Row, SwitchRow, TabHeader } from "./parts";

type Status = "off" | "connecting" | "connected" | "no-discord" | "error";

function useDiscordStatus(): { status: Status; user: DiscordUser | null; error: string | null } {
  const [state, setState] = useState<{ status: Status; user: DiscordUser | null; error: string | null }>({ status: "off", user: null, error: null });
  useEffect(() => {
    const tick = () => void window.evplayer?.discord.status().then(setState);
    tick();
    const timer = setInterval(tick, 2500);
    return () => clearInterval(timer);
  }, []);
  return state;
}

/** how the activity will look on your profile */
function Preview({ name }: { name: string }) {
  const track = usePlayer((s) => currentTrack(s));
  const playing = usePlayer((s) => s.status === "playing");
  const pos = usePlayer((s) => Math.floor(s.positionMs / 1000) * 1000);
  const dur = usePlayer((s) => s.durationMs);
  const o = useSettings();
  const shown = track && (playing || o.discordShowPaused);
  const statusLine = !track ? "" : o.discordStatusShows === "app" ? "EvPlayer" : o.discordStatusShows === "artist" ? artistLine(track.artists) : track.title;
  return (
    <div className="dc-preview">
      <div className="dc-member">
        <span className="dc-avatar small"><IconUser size={14} /></span>
        <span className="dc-member-text">
          <span className="dc-member-name">{name}</span>
          <span className="dc-member-status">{shown ? <>Слушает <b>{statusLine}</b></> : "—"}</span>
        </span>
      </div>
      <div className="dc-activity">
        <span className="dc-activity-label">Слушает EvPlayer</span>
        {shown ? (
          <div className="dc-activity-body">
            <span className="dc-art">{o.discordShowArtwork && track.artwork ? <img src={track.artwork} alt="" /> : <IconMusic size={26} />}</span>
            <span className="dc-activity-text">
              <span className="dc-title">{track.title}</span>
              <span className="dc-sub">{artistLine(track.artists)}</span>
              {o.discordShowTimeline && playing && dur > 0 && (
                <span className="dc-timeline">
                  <span>{formatTime(pos)}</span>
                  <span className="dc-bar"><span style={{ width: `${Math.min(100, (pos / dur) * 100)}%` }} /></span>
                  <span>{formatTime(dur)}</span>
                </span>
              )}
              {!playing && <span className="dc-sub">на паузе</span>}
            </span>
          </div>
        ) : (
          <p className="muted small">{track ? "На паузе статус скрыт" : "Включите трек — здесь появится, как его видят друзья"}</p>
        )}
        {shown && o.discordShowButton && track.permalink && <span className="dc-button">Слушать</span>}
      </div>
    </div>
  );
}

export default function DiscordTab() {
  const o = useSettings();
  const { status, user, error } = useDiscordStatus();
  const hasId = Boolean(discordAppId(o));
  const [showSetup, setShowSetup] = useState(!hasId);
  const tone = status === "connected" ? "ok" : status === "off" ? "off" : "warn";
  const label = { off: "выключено", connecting: "подключаюсь…", connected: "подключено", "no-discord": "Discord не запущен", error: "ошибка" }[status];
  const name = user?.globalName || user?.username || "Вы";

  return (
    <>
      <TabHeader icon={<IconChat size={22} />} hue={235} title="Discord" description="Показывайте друзьям, что сейчас играет: трек, обложка и время прямо в профиле." />
      <Card>
        <div className="dc-account">
          <span className="dc-avatar">{user?.avatar ? <img src={user.avatar} alt="" /> : <IconUser size={26} />}</span>
          <span className="dc-account-text">
            <span className="dc-account-name">{status === "connected" ? name : "Discord"}</span>
            <span className="muted small">{status === "connected" && user ? `@${user.username}` : "Нужен запущенный Discord для компьютера"}</span>
          </span>
          <Pill tone={tone}>{label}</Pill>
          <input type="checkbox" role="switch" className="switch" aria-label="Статус в Discord" checked={o.discordEnabled} disabled={!hasId} onChange={(e) => o.set({ discordEnabled: e.target.checked })} />
        </div>
        {!hasId && <p className="muted small">Сначала один раз подключите приложение ниже — это займёт минуту.</p>}
        {hasId && o.discordEnabled && status === "no-discord" && <p className="muted small">Не вижу Discord. Запустите приложение Discord для компьютера (не браузерную версию) — EvPlayer подключится сам.</p>}
        {error && <p className="field-error">Discord: {error === "Invalid Client ID" ? "такого Application ID нет — проверьте его ниже" : error}</p>}
      </Card>

      <div className="dc-grid">
        <Card title="Что видно">
          <SwitchRow title="Обложка" hint="Картинка трека рядом со статусом" checked={o.discordShowArtwork} onChange={(v) => o.set({ discordShowArtwork: v })} />
          <SwitchRow title="Время" hint="Полоска прогресса и сколько осталось" checked={o.discordShowTimeline} onChange={(v) => o.set({ discordShowTimeline: v })} />
          <SwitchRow title="Кнопка «Слушать»" hint="Друзья откроют трек на SoundCloud или YouTube" checked={o.discordShowButton} onChange={(v) => o.set({ discordShowButton: v })} />
          <SwitchRow title="Показывать на паузе" hint="Иначе статус исчезает, пока музыка стоит" checked={o.discordShowPaused} onChange={(v) => o.set({ discordShowPaused: v })} />
          <Row title="В списке участников" hint="Что написано под вашим именем на сервере">
            <Segmented<DiscordStatusShows>
              label="В списке участников"
              value={o.discordStatusShows}
              onChange={(v) => o.set({ discordStatusShows: v })}
              options={[
                { value: "song", label: "Трек" },
                { value: "artist", label: "Исполнитель" },
                { value: "app", label: "EvPlayer" },
              ]}
            />
          </Row>
        </Card>
        <Card title="Так увидят друзья">
          <Preview name={status === "connected" ? name : "Вы"} />
        </Card>
      </div>

      <Card
        title="Подключение приложения"
        hint={DEFAULT_DISCORD_APP_ID ? "В этой сборке уже есть приложение EvPlayer — менять ничего не нужно." : "Discord показывает статус от имени приложения. Его создают один раз, бесплатно."}
        actions={<button type="button" className="btn ghost" onClick={() => setShowSetup((v) => !v)}>{showSetup ? "Скрыть" : "Настроить"}</button>}
      >
        {showSetup && (
          <>
            <ol className="steps">
              <li>Откройте <button type="button" className="link-btn strong" onClick={() => window.open("https://discord.com/developers/applications", "_blank")}>discord.com/developers/applications</button> и нажмите «New Application».</li>
              <li>Назовите его <b>EvPlayer</b> — это имя появится в «Слушает …».</li>
              <li>На странице «General Information» скопируйте <b>Application ID</b> и вставьте сюда.</li>
            </ol>
            <label className="field">
              <span>Application ID</span>
              <input value={o.discordAppId} onChange={(e) => o.set({ discordAppId: e.target.value.replace(/\D/g, "") })} placeholder={DEFAULT_DISCORD_APP_ID || "1234567890123456789"} spellCheck={false} />
            </label>
          </>
        )}
      </Card>
    </>
  );
}
