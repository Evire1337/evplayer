import { useEffect, useState } from "react";
import QRCode from "qrcode";
import Dialog from "@/ui/components/Dialog";
import { Segmented, Toggle } from "@/ui/components/Controls";
import {
  IconCheck,
  IconClose,
  IconCopy,
  IconJam,
  IconLink,
  IconQr,
} from "@/ui/components/Icons";
import { toast } from "@/ui/toasts";
import { jamEngine } from "./engine";
import { useJam } from "./store";
import {
  JAM_AVATAR_COLORS,
  JAM_REACTIONS,
  type JamControlMode,
} from "./types";

export default function JamDialog() {
  const isOpen = useJam((s) => s.isOpen);
  const active = useJam((s) => s.active);
  const status = useJam((s) => s.status);
  const isHost = useJam((s) => s.isHost);
  const roomCode = useJam((s) => s.roomCode);
  const hostName = useJam((s) => s.hostName);
  const controlMode = useJam((s) => s.controlMode);
  const participants = useJam((s) => s.participants);
  const myProfile = useJam((s) => s.myProfile);
  const setProfile = useJam((s) => s.setProfile);
  const setOpen = useJam((s) => s.setOpen);

  const [tab, setTab] = useState<"host" | "join">("host");
  const [joinCode, setJoinCode] = useState("");
  const [hostControlMode, setHostControlMode] = useState<JamControlMode>("anyone");
  const [showQr, setShowQr] = useState(false);
  const [qrDataUrl, setQrDataUrl] = useState("");
  const [chatInput, setChatInput] = useState("");

  // Generate QR code when active
  useEffect(() => {
    if (active && roomCode) {
      const shareUrl = `evplayer://jam/${roomCode}`;
      QRCode.toDataURL(shareUrl, {
        margin: 1,
        width: 180,
        color: { dark: "#06201d", light: "#ffffff" },
      })
        .then(setQrDataUrl)
        .catch(() => setQrDataUrl(""));
    }
  }, [active, roomCode]);

  const handleStartHost = async () => {
    try {
      await jamEngine.startHost(hostControlMode);
    } catch {
      // handled in engine
    }
  };

  const handleJoin = async () => {
    if (!joinCode.trim()) {
      toast("Введите код сессии");
      return;
    }
    try {
      await jamEngine.joinSession(joinCode.trim());
    } catch {
      // handled in engine
    }
  };

  const copyCode = () => {
    void navigator.clipboard.writeText(roomCode).then(() => {
      toast("Код сессии скопирован");
    });
  };

  const copyLink = () => {
    const link = `evplayer://jam/${roomCode}`;
    void navigator.clipboard.writeText(link).then(() => {
      toast("Ссылка для присоединения скопирована");
    });
  };

  const handleSendChat = (e: React.FormEvent) => {
    e.preventDefault();
    if (!chatInput.trim()) return;
    jamEngine.sendChat(chatInput);
    setChatInput("");
  };

  return (
    <Dialog title="Совместное прослушивание" open={isOpen} onClose={() => setOpen(false)} wide>
      {!active ? (
        <div className="jam-dialog-body">
          <p className="jam-intro">
            Слушайте музыку вместе с друзьями в реальном времени, как в <strong>Spotify Jam</strong>. Синхронное воспроизведение, общая очередь и живые реакции!
          </p>

          <div className="jam-profile-section">
            <label className="jam-field-label" htmlFor="jam-user-name">Ваш профиль в сессии</label>
            <div className="jam-profile-row">
              <input
                id="jam-user-name"
                type="text"
                className="input jam-name-input"
                placeholder="Ваше имя"
                maxLength={24}
                value={myProfile.name}
                onChange={(e) => setProfile({ name: e.target.value })}
              />
              <div className="jam-colors" aria-label="Цвет аватара">
                {JAM_AVATAR_COLORS.map((color) => (
                  <button
                    key={color}
                    type="button"
                    className={`jam-color-dot${myProfile.avatarColor === color ? " is-selected" : ""}`}
                    style={{ backgroundColor: color }}
                    onClick={() => setProfile({ avatarColor: color })}
                    aria-label={`Выбрать цвет ${color}`}
                  >
                    {myProfile.avatarColor === color && <IconCheck size={12} />}
                  </button>
                ))}
              </div>
            </div>
          </div>

          <Segmented
            label="Действие"
            value={tab}
            onChange={(v) => setTab(v)}
            options={[
              { value: "host", label: "Создать сессию" },
              { value: "join", label: "Присоединиться" },
            ]}
          />

          {tab === "host" ? (
            <div className="jam-tab-content">
              <div className="jam-setting-box">
                <Toggle
                  id="jam-ctrl-mode"
                  title="Все участники могут управлять музыкой"
                  hint="Если выключено — только вы можете переключать треки и ставить на паузу, а друзья только добавляют музыку в очередь."
                  checked={hostControlMode === "anyone"}
                  onChange={(checked) => setHostControlMode(checked ? "anyone" : "host_only")}
                />
              </div>

              <button
                type="button"
                className="btn primary jam-action-btn"
                disabled={status === "hosting"}
                onClick={handleStartHost}
              >
                {status === "hosting" ? <span className="spinner" /> : <><IconJam size={18} /> Создать сессию Jam</>}
              </button>
            </div>
          ) : (
            <div className="jam-tab-content">
              <div className="jam-field">
                <label className="jam-field-label" htmlFor="jam-join-input">Код сессии</label>
                <input
                  id="jam-join-input"
                  type="text"
                  className="input jam-code-input"
                  placeholder="Например: JAM-7K9P"
                  value={joinCode}
                  maxLength={16}
                  onChange={(e) => setJoinCode(e.target.value.toUpperCase())}
                  onKeyDown={(e) => e.key === "Enter" && handleJoin()}
                />
                <span className="jam-field-hint">Введите 4-буквенный код или полный код JAM-XXXX от друга</span>
              </div>

              <button
                type="button"
                className="btn primary jam-action-btn"
                disabled={status === "joining"}
                onClick={handleJoin}
              >
                {status === "joining" ? <span className="spinner" /> : "Присоединиться к сессии"}
              </button>
            </div>
          )}
        </div>
      ) : (
        <div className="jam-dialog-body jam-session-active">
          {/* Active session top card */}
          <div className="jam-room-card glass">
            <div className="jam-room-status">
              <span className="jam-live-pulse" />
              <div className="jam-room-meta">
                <span className="jam-room-state-text">Сессия активна</span>
                <span className="jam-room-host-sub">
                  {isHost ? "Вы организатор сессии" : `Организатор: ${hostName}`}
                </span>
              </div>
            </div>

            <div className="jam-code-display">
              <span className="jam-code-text">{roomCode}</span>
              <div className="jam-code-buttons">
                <button
                  type="button"
                  className="round-btn soft"
                  aria-label="Копировать код"
                  data-tip="Копировать код"
                  onClick={copyCode}
                >
                  <IconCopy size={17} />
                </button>
                <button
                  type="button"
                  className="round-btn soft"
                  aria-label="Копировать ссылку"
                  data-tip="Копировать ссылку"
                  onClick={copyLink}
                >
                  <IconLink size={17} />
                </button>
                <button
                  type="button"
                  className={`round-btn soft${showQr ? " is-on" : ""}`}
                  aria-label="QR-код для подключения"
                  data-tip="QR-код для подключения"
                  onClick={() => setShowQr(!showQr)}
                >
                  <IconQr size={17} />
                </button>
              </div>
            </div>

            {showQr && qrDataUrl && (
              <div className="jam-qr-container">
                <img src={qrDataUrl} alt="QR-код для подключения" className="jam-qr-img" />
                <span className="jam-field-hint">Отсканируйте код для быстрого входа</span>
              </div>
            )}
          </div>

          {/* Quick Reaction buttons */}
          <div className="jam-section">
            <span className="jam-section-title">Быстрые реакции</span>
            <div className="jam-reactions-row">
              {JAM_REACTIONS.map((emoji) => (
                <button
                  key={emoji}
                  type="button"
                  className="jam-reaction-btn"
                  onClick={() => jamEngine.sendReaction(emoji)}
                  aria-label={`Отправить ${emoji}`}
                >
                  {emoji}
                </button>
              ))}
            </div>
          </div>

          {/* Participants list */}
          <div className="jam-section">
            <span className="jam-section-title">
              Участники ({participants.length})
            </span>
            <div className="jam-participants-list">
              {participants.map((p) => {
                const isMe = isHost ? p.isHost : p.name === myProfile.name;
                return (
                  <div key={p.id} className="jam-participant-item">
                    <span
                      className="jam-participant-avatar"
                      style={{ backgroundColor: p.avatarColor }}
                    >
                      {p.name.slice(0, 1).toUpperCase()}
                    </span>
                    <span className="jam-participant-name">
                      {p.name}
                      {isMe && <span className="jam-badge self">Вы</span>}
                      {p.isHost && <span className="jam-badge host">👑 Хост</span>}
                    </span>

                    {isHost && !p.isHost && (
                      <button
                        type="button"
                        className="jam-kick-btn"
                        aria-label={`Исключить ${p.name}`}
                        data-tip="Исключить из сессии"
                        onClick={() => jamEngine.kickParticipant(p.id)}
                      >
                        <IconClose size={14} />
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          {/* Host session settings */}
          {isHost && (
            <div className="jam-section jam-host-controls">
              <Toggle
                id="jam-host-ctrl-toggle"
                title="Все участники управляют музыкой"
                hint={controlMode === "anyone" ? "Участники могут переключать треки и ставить на паузу" : "Только вы управляете воспроизведением, другие только добавляют в очередь"}
                checked={controlMode === "anyone"}
                onChange={(checked) => jamEngine.setControlMode(checked ? "anyone" : "host_only")}
              />
            </div>
          )}

          {/* Quick chat message */}
          <form className="jam-chat-form" onSubmit={handleSendChat}>
            <input
              type="text"
              className="input jam-chat-input"
              placeholder="Отправить сообщение участникам…"
              maxLength={150}
              value={chatInput}
              onChange={(e) => setChatInput(e.target.value)}
            />
            <button type="submit" className="btn soft" disabled={!chatInput.trim()}>
              Отправить
            </button>
          </form>

          {/* Leave / End button */}
          <div className="jam-footer-actions">
            <button
              type="button"
              className="btn danger jam-leave-btn"
              onClick={() => jamEngine.leaveOrEndSession()}
            >
              {isHost ? "Завершить сессию для всех" : "Покинуть сессию"}
            </button>
          </div>
        </div>
      )}
    </Dialog>
  );
}
