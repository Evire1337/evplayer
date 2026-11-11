import { useEffect, useRef, useState } from "react";
import Dialog from "@/ui/components/Dialog";
import { Segmented, Toggle } from "@/ui/components/Controls";
import { IconCheck, IconCopy, IconJam, IconLink, IconQr, IconRefresh } from "@/ui/components/Icons";
import { toast } from "@/ui/toasts";
import { jam } from "./engine";
import { useJam } from "./store";
import { inviteLink } from "./protocol";
import { JAM_COLORS, JAM_REACTIONS, type JamPerson } from "./types";
import { listeners } from "./JamBanner";
import Emoji from "./Emoji";

export default function JamDialog() {
  const open = useJam((s) => s.open);
  const status = useJam((s) => s.status);
  const setOpen = useJam((s) => s.setOpen);
  return (
    <Dialog title="Совместное прослушивание" open={open} onClose={() => setOpen(false)} wide>
      {status === "live" ? <Session /> : <Start />}
    </Dialog>
  );
}

function copy(text: string, done: string): void {
  void navigator.clipboard.writeText(text).then(() => toast(done));
}

function Start() {
  const status = useJam((s) => s.status);
  const waiting = useJam((s) => s.waiting);
  const name = useJam((s) => s.name);
  const color = useJam((s) => s.color);
  const setProfile = useJam((s) => s.setProfile);
  const [tab, setTab] = useState<"host" | "join">("host");
  const [code, setCode] = useState("");
  const busy = status === "starting" || status === "joining";

  if (waiting) {
    return (
      <div className="jam-body">
        <div className="set-card jam-waiting">
          <span className="spinner small-spin" />
          <span>Организатор решает, пустить ли вас в сессию {useJam.getState().code}…</span>
        </div>
        <div className="dialog-actions">
          <button type="button" className="btn ghost" onClick={() => void jam.leave()}>Не ждать</button>
        </div>
      </div>
    );
  }

  return (
    <div className="jam-body">
      <p className="muted">Слушайте вместе с друзьями: одна очередь на всех, музыка играет одновременно, а в чат можно писать и слать реакции.</p>
      <div className="set-card jam-profile">
        <span className="jam-label">Как вас увидят</span>
        <div className="jam-profile-row">
          <input className="filter-input jam-name" placeholder="Ваше имя" maxLength={24} value={name} onChange={(e) => setProfile({ name: e.target.value })} />
          <div className="jam-colors" aria-label="Цвет">
            {JAM_COLORS.map((c) => (
              <button key={c} type="button" className={`jam-color${color === c ? " is-on" : ""}`} style={{ background: c }} onClick={() => setProfile({ color: c })} aria-label={`Цвет ${c}`}>
                {color === c && <IconCheck size={12} />}
              </button>
            ))}
          </div>
        </div>
      </div>
      <Segmented label="Что сделать" value={tab} onChange={setTab} options={[{ value: "host", label: "Создать сессию" }, { value: "join", label: "Присоединиться" }]} />
      {tab === "host" ? (
        <>
          <SessionSettings />
          <div className="dialog-actions">
            <button type="button" className="btn primary" disabled={busy} onClick={() => void jam.start()}>
              {status === "starting" ? <span className="spinner small-spin" /> : <><IconJam size={18} /> Создать сессию</>}
            </button>
          </div>
        </>
      ) : (
        <>
          <label className="field">
            <span>Код или ссылка приглашения</span>
            <input
              className="filter-input jam-code-input"
              placeholder="JAM-7K9P-2QXZ"
              maxLength={60}
              value={code}
              onChange={(e) => setCode(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void jam.join(code)}
            />
          </label>
          <div className="dialog-actions">
            <button type="button" className="btn primary" disabled={busy || !code.trim()} onClick={() => void jam.join(code)}>
              {status === "joining" ? <span className="spinner small-spin" /> : "Присоединиться"}
            </button>
          </div>
          <PublicList busy={busy} />
        </>
      )}
    </div>
  );
}

const ACCESS_HINT = {
  open: "Заходит любой, у кого есть код, ссылка или QR-код",
  ask: "Каждый, кто пришёл по коду, ждёт, пока вы его пустите",
  closed: "Новые люди не войдут ни по коду, ни по ссылке",
} as const;

/** how the session is set up: before it starts (remembered) and, for the host, while it is on */
function SessionSettings() {
  const live = useJam((s) => s.status === "live");
  const control = useJam((s) => (live ? s.control : s.setup.control));
  const access = useJam((s) => (live ? s.access : s.setup.access));
  const listed = useJam((s) => (live ? s.listed : s.setup.listed));
  return (
    <div className="set-card jam-settings">
      <Toggle
        id="jam-control"
        title="Все участники управляют музыкой"
        hint={control === "anyone" ? "Друзья включают треки, переключают их и ставят паузу" : "Управляете только вы; друзья добавляют песни в очередь, а своё у них не играет"}
        checked={control === "anyone"}
        onChange={(on) => jam.setControl(on ? "anyone" : "host")}
      />
      <div className="jam-setting">
        <span className="jam-setting-title">Вход</span>
        <Segmented label="Вход" value={access} onChange={(a) => jam.setAccess(a)} options={[{ value: "open", label: "Открыт" }, { value: "ask", label: "По запросу" }, { value: "closed", label: "Закрыт" }]} />
        <span className="muted small">{ACCESS_HINT[access]}</span>
      </div>
      <Toggle
        id="jam-listed"
        title="Показывать в общем списке"
        hint={access === "closed" ? "Закрытая сессия в списке не видна" : "Сессию увидят все, кто открыл «Присоединиться», вместе с числом участников"}
        checked={listed}
        onChange={(on) => jam.setListed(on)}
      />
    </div>
  );
}

/** open sessions others put in the public list (looked at only while this is on screen) */
function PublicList({ busy }: { busy: boolean }) {
  const browse = useJam((s) => s.browse);
  const lobby = useJam((s) => s.lobby);
  useEffect(() => {
    if (!browse) return;
    void jam.browse(true);
    return () => void jam.browse(false);
  }, [browse]);
  return (
    <div className="jam-public">
      <Toggle id="jam-browse" title="Открытые сессии" hint="Сессии, которые их организаторы показали всем" checked={browse} onChange={(on) => useJam.setState({ browse: on })} />
      {browse && (
        <div className="jam-public-list">
          {lobby.length === 0 && <p className="muted small jam-public-empty">Пока никого: открытые сессии появятся здесь сами</p>}
          {lobby.map((l) => (
            <div key={l.code} className="jam-public-item">
              <span className="jam-public-text">
                <span className="jam-public-host">{l.host}</span>
                <span className="muted small">{listeners(l.people)}{l.song ? ` · ${l.song}` : ""}</span>
              </span>
              {l.access === "ask" && <span className="jam-badge">по запросу</span>}
              <button type="button" className="btn primary" disabled={busy} onClick={() => void jam.join(l.code)}>Войти</button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** people waiting to be let in, for the host */
function Knocks() {
  const knocks = useJam((s) => s.knocks);
  if (!knocks.length) return null;
  return (
    <div className="jam-knocks">
      <span className="jam-label">Хотят войти</span>
      {knocks.map((k) => (
        <div key={k.id} className="jam-knock">
          <span className="jam-avatar" style={{ background: k.color }}>{k.name.slice(0, 1).toUpperCase()}</span>
          <span className="jam-person-name">{k.name}</span>
          <button type="button" className="btn primary" onClick={() => jam.admit(k.id)}>Пустить</button>
          <button type="button" className="btn ghost" onClick={() => jam.turnAway(k.id)}>Нет</button>
        </div>
      ))}
    </div>
  );
}
function Session() {
  const host = useJam((s) => s.host);
  const code = useJam((s) => s.code);
  const people = useJam((s) => s.people);
  const setOpen = useJam((s) => s.setOpen);
  const [showQr, setShowQr] = useState(false);
  const [qr, setQr] = useState("");
  const [changing, setChanging] = useState(false);
  const hostName = people.find((p) => p.host)?.name ?? "";

  useEffect(() => {
    if (!showQr) return;
    let live = true;
    // the QR code maker loads only when a QR code is asked for
    import("qrcode")
      .then(({ default: QRCode }) => QRCode.toDataURL(inviteLink(code), { margin: 1, width: 320, color: { dark: "#06201d", light: "#eef7f4" } }))
      .then((url) => live && setQr(url))
      .catch(() => live && setQr(""));
    return () => {
      live = false;
    };
  }, [showQr, code]);

  const changeCode = async () => {
    setChanging(true);
    await jam.changeCode();
    setChanging(false);
  };

  return (
    <div className="jam-body">
      <div className="set-card jam-room">
        <div className="jam-room-head">
          <div className="jam-room-status">
            <span className="jam-pulse" />
            <div className="jam-room-meta">
              <span className="jam-room-live">Сессия идёт</span>
              <span className="jam-room-host">{host ? "Вы организатор" : `Организатор: ${hostName}`}</span>
            </div>
          </div>
          <div className="jam-code">
            <span className="jam-code-text">{code}</span>
            <button type="button" className="icon-btn" aria-label="Копировать код" data-tip="Копировать код" onClick={() => copy(code, "Код скопирован")}>
              <IconCopy size={16} />
            </button>
            <button type="button" className="icon-btn" aria-label="Копировать ссылку" data-tip="Копировать ссылку" onClick={() => copy(inviteLink(code), "Ссылка-приглашение скопирована")}>
              <IconLink size={16} />
            </button>
            <button type="button" className={`icon-btn${showQr ? " is-active" : ""}`} aria-label="QR-код" data-tip="QR-код" onClick={() => setShowQr(!showQr)}>
              <IconQr size={16} />
            </button>
            {host && (
              <button type="button" className="icon-btn" disabled={changing} aria-label="Новый код, ссылка и QR" data-tip="Новый код, ссылка и QR: старые перестанут работать" onClick={() => void changeCode()}>
                <IconRefresh size={16} />
              </button>
            )}
          </div>
        </div>
        {showQr && qr && (
          <div className="jam-qr">
            <img src={qr} alt="QR-код приглашения" />
            <span className="muted small">Наведите камеру телефона с EvPlayer</span>
          </div>
        )}
      </div>

      <div className="jam-reactions" aria-label="Реакции">
        {JAM_REACTIONS.map((emoji) => (
          <button key={emoji} type="button" className="jam-reaction" onClick={() => jam.react(emoji)} aria-label={`Отправить ${emoji}`}>
            <Emoji emoji={emoji} size={24} />
          </button>
        ))}
      </div>

      <div className="jam-columns">
        <div className="jam-column">
          <span className="jam-label">Участники · {people.length}</span>
          <div className="jam-people">
            {people.map((p) => <Person key={p.id} person={p} canKick={host && !p.host} />)}
          </div>
        </div>
        <div className="jam-column">
          <span className="jam-label">Чат</span>
          <Chat />
        </div>
      </div>

      {host && (
        <>
          <Knocks />
          <SessionSettings />
        </>
      )}

      <div className="dialog-actions">
        <button type="button" className="btn ghost" onClick={() => setOpen(false)}>Свернуть</button>
        <button type="button" className="btn danger" onClick={() => void jam.leave()}>{host ? "Завершить сессию" : "Выйти из сессии"}</button>
      </div>
    </div>
  );
}

/** a person in the list; for the host, a click on someone else removes them */
function Person({ person, canKick }: { person: JamPerson; canKick: boolean }) {
  const me = person.id === jam.myId;
  const body = (
    <>
      <span className="jam-avatar" style={{ background: person.color }}>{person.name.slice(0, 1).toUpperCase()}</span>
      <span className="jam-person-text">
        <span className="jam-person-name">{person.name}</span>
        <span className="jam-badges">
          {person.host && <span className="jam-badge is-host">Организатор</span>}
          {me && <span className="jam-badge">Вы</span>}
        </span>
      </span>
    </>
  );
  if (!canKick) return <div className="jam-person">{body}</div>;
  return (
    <button type="button" className="jam-person is-kickable" onClick={() => jam.kick(person.id)} aria-label={`Исключить ${person.name}`}>
      {body}
      <span className="jam-kick">Исключить</span>
    </button>
  );
}

function Chat() {
  const chat = useJam((s) => s.chat);
  const myName = useJam((s) => s.name);
  const [text, setText] = useState("");
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const list = listRef.current;
    if (list) list.scrollTop = list.scrollHeight;
  }, [chat.length]);

  const send = (e: React.FormEvent) => {
    e.preventDefault();
    jam.sendChat(text);
    setText("");
  };

  return (
    <div className="jam-chat">
      <div className="jam-chat-list" ref={listRef}>
        {chat.length === 0 && <p className="muted small jam-chat-empty">Напишите что-нибудь или пришлите реакцию</p>}
        {chat.map((m) => (
          <div key={m.id} className={`jam-message${m.from === myName ? " is-mine" : ""}`}>
            <span className="jam-message-head">
              <span className="jam-message-from">{m.from}</span>
              <span className="jam-message-time">{new Date(m.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
            </span>
            <span className="jam-message-text">{m.text}</span>
          </div>
        ))}
      </div>
      <form className="jam-chat-form" onSubmit={send}>
        <input className="filter-input" placeholder="Сообщение…" maxLength={200} value={text} onChange={(e) => setText(e.target.value)} />
        <button type="submit" className="btn primary" disabled={!text.trim()}>Отправить</button>
      </form>
    </div>
  );
}
