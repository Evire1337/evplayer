import { useEffect, useRef, useState } from "react";
import Dialog from "@/ui/components/Dialog";
import { Segmented, Toggle } from "@/ui/components/Controls";
import { IconCheck, IconCopy, IconJam, IconLink, IconQr, IconRefresh } from "@/ui/components/Icons";
import { toast } from "@/ui/toasts";
import { jam } from "./engine";
import { useJam } from "./store";
import { inviteLink } from "./protocol";
import { JAM_COLORS, JAM_REACTIONS, type JamPerson } from "./types";

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
  const name = useJam((s) => s.name);
  const color = useJam((s) => s.color);
  const control = useJam((s) => s.control);
  const setProfile = useJam((s) => s.setProfile);
  const [tab, setTab] = useState<"host" | "join">("host");
  const [code, setCode] = useState("");
  const busy = status === "starting" || status === "joining";

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
          <div className="set-card">
            <Toggle
              id="jam-control"
              title="Все участники управляют музыкой"
              hint="Если выключено, переключать треки и ставить паузу можете только вы, а друзья добавляют песни в очередь."
              checked={control === "anyone"}
              onChange={(on) => useJam.setState({ control: on ? "anyone" : "host" })}
            />
          </div>
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
              placeholder="JAM-7K9P2Q"
              maxLength={40}
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
        </>
      )}
    </div>
  );
}

function Session() {
  const host = useJam((s) => s.host);
  const code = useJam((s) => s.code);
  const control = useJam((s) => s.control);
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
            {emoji}
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
        <div className="set-card">
          <Toggle
            id="jam-control-live"
            title="Все участники управляют музыкой"
            hint={control === "anyone" ? "Друзья переключают треки и ставят паузу" : "Управляете только вы, друзья добавляют песни в очередь"}
            checked={control === "anyone"}
            onChange={(on) => jam.setControl(on ? "anyone" : "host")}
          />
        </div>
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
