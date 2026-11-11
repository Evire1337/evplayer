import { useEffect, useRef, useState } from "react";
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
  const chatMessages = useJam((s) => s.chatMessages);
  const setProfile = useJam((s) => s.setProfile);
  const setOpen = useJam((s) => s.setOpen);

  const [tab, setTab] = useState<"host" | "join">("host");
  const [joinCode, setJoinCode] = useState("");
  const [hostControlMode, setHostControlMode] = useState<JamControlMode>("anyone");
  const [showQr, setShowQr] = useState(false);
  const [qrDataUrl, setQrDataUrl] = useState("");
  const [chatInput, setChatInput] = useState("");
  const [lastClickedEmoji, setLastClickedEmoji] = useState<string | null>(null);

  const chatEndRef = useRef<HTMLDivElement | null>(null);

  // Auto-scroll chat to latest message
  useEffect(() => {
    if (chatEndRef.current) {
      chatEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [chatMessages.length, isOpen]);

  // Generate QR code when active
  useEffect(() => {
    if (active && roomCode) {
      const shareUrl = `evplayer://jam/${roomCode}`;
      QRCode.toDataURL(shareUrl, {
        margin: 1,
        width: 180,
        color: { dark: "#06201d", light: "#eef7f4" },
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

  const handleReactionClick = (emoji: string) => {
    setLastClickedEmoji(emoji);
    jamEngine.sendReaction(emoji);
    setTimeout(() => setLastClickedEmoji(null), 400);
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
            Слушайте музыку синхронно с друзьями в реальном времени. Общая очередь воспроизведения, мгновенное переключение, живые реакции и чат.
          </p>

          <div className="set-card jam-profile-card">
            <span className="jam-card-label">Ваш профиль в сессии</span>
            <div className="jam-profile-row">
              <input
                id="jam-user-name"
                type="text"
                className="filter-input jam-name-input"
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
              <div className="set-card jam-setting-box">
                <Toggle
                  id="jam-ctrl-mode"
                  title="Все участники могут управлять музыкой"
                  hint="Если выключено — только вы можете переключать треки и ставить на паузу, а друзья только добавляют треки в очередь."
                  checked={hostControlMode === "anyone"}
                  onChange={(checked) => setHostControlMode(checked ? "anyone" : "host_only")}
                />
              </div>

              <div className="dialog-actions">
                <button
                  type="button"
                  className="btn primary jam-action-btn"
                  disabled={status === "hosting"}
                  onClick={handleStartHost}
                >
                  {status === "hosting" ? (
                    <span className="spinner small-spin" />
                  ) : (
                    <>
                      <IconJam size={18} /> Создать сессию Jam
                    </>
                  )}
                </button>
              </div>
            </div>
          ) : (
            <div className="jam-tab-content">
              <div className="set-card jam-setting-box">
                <label className="field">
                  <span>Код сессии</span>
                  <input
                    id="jam-join-input"
                    type="text"
                    className="filter-input jam-code-input"
                    placeholder="Например: JAM-7K9P"
                    value={joinCode}
                    maxLength={16}
                    onChange={(e) => setJoinCode(e.target.value.toUpperCase())}
                    onKeyDown={(e) => e.key === "Enter" && handleJoin()}
                  />
                  <span className="jam-field-hint">Введите код комнаты или код из ссылки приглашения</span>
                </label>
              </div>

              <div className="dialog-actions">
                <button
                  type="button"
                  className="btn primary jam-action-btn"
                  disabled={status === "joining"}
                  onClick={handleJoin}
                >
                  {status === "joining" ? <span className="spinner small-spin" /> : "Присоединиться"}
                </button>
              </div>
            </div>
          )}
        </div>
      ) : (
        <div className="jam-dialog-body jam-session-active">
          {/* Active session top card */}
          <div className="set-card jam-room-card">
            <div className="jam-room-header">
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
                    className="icon-btn"
                    aria-label="Копировать код"
                    data-tip="Копировать код"
                    onClick={copyCode}
                  >
                    <IconCopy size={16} />
                  </button>
                  <button
                    type="button"
                    className="icon-btn"
                    aria-label="Копировать ссылку"
                    data-tip="Копировать ссылку"
                    onClick={copyLink}
                  >
                    <IconLink size={16} />
                  </button>
                  <button
                    type="button"
                    className={`icon-btn${showQr ? " is-active" : ""}`}
                    aria-label="QR-код для подключения"
                    data-tip="QR-код для подключения"
                    onClick={() => setShowQr(!showQr)}
                  >
                    <IconQr size={16} />
                  </button>
                </div>
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
          <div className="jam-reactions-bar">
            <span className="jam-section-subtitle">Быстрые реакции:</span>
            <div className="jam-reactions-row">
              {JAM_REACTIONS.map((emoji) => (
                <button
                  key={emoji}
                  type="button"
                  className={`jam-reaction-btn${lastClickedEmoji === emoji ? " is-clicked" : ""}`}
                  onClick={() => handleReactionClick(emoji)}
                  aria-label={`Отправить ${emoji}`}
                >
                  {emoji}
                </button>
              ))}
            </div>
          </div>

          {/* Grid layout: Participants on left, Live Chat on right */}
          <div className="jam-columns-layout">
            {/* Participants column */}
            <div className="jam-column jam-participants-column">
              <span className="jam-section-subtitle">Участники ({participants.length})</span>
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
                      <div className="jam-participant-info">
                        <span className="jam-participant-name">{p.name}</span>
                        <div className="jam-participant-badges">
                          {p.isHost && <span className="jam-badge host">👑 Организатор</span>}
                          {isMe && <span className="jam-badge self">Вы</span>}
                        </div>
                      </div>

                      {isHost && !p.isHost && (
                        <button
                          type="button"
                          className="icon-btn jam-kick-btn"
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

            {/* Live Chat column */}
            <div className="jam-column jam-chat-column">
              <span className="jam-section-subtitle">Чат сессии</span>
              <div className="jam-chat-box">
                <div className="jam-chat-messages">
                  {chatMessages.length === 0 ? (
                    <div className="jam-chat-empty">
                      Напишите сообщение или отправьте реакцию участникам!
                    </div>
                  ) : (
                    chatMessages.map((m) => {
                      const isMyMsg = m.fromName === myProfile.name;
                      const timeStr = new Date(m.timestamp).toLocaleTimeString([], {
                        hour: "2-digit",
                        minute: "2-digit",
                      });
                      return (
                        <div
                          key={m.id}
                          className={`jam-chat-item${isMyMsg ? " is-self" : ""}`}
                        >
                          <div className="jam-chat-item-header">
                            <span className="jam-chat-item-author">{m.fromName}</span>
                            <span className="jam-chat-item-time">{timeStr}</span>
                          </div>
                          <div className="jam-chat-item-body">{m.text}</div>
                        </div>
                      );
                    })
                  )}
                  <div ref={chatEndRef} />
                </div>

                <form className="jam-chat-form" onSubmit={handleSendChat}>
                  <input
                    type="text"
                    className="filter-input jam-chat-input"
                    placeholder="Написать сообщение…"
                    maxLength={150}
                    value={chatInput}
                    onChange={(e) => setChatInput(e.target.value)}
                  />
                  <button
                    type="submit"
                    className="btn primary jam-chat-send-btn"
                    disabled={!chatInput.trim()}
                  >
                    Отправить
                  </button>
                </form>
              </div>
            </div>
          </div>

          {/* Host session settings */}
          {isHost && (
            <div className="set-card jam-host-controls">
              <Toggle
                id="jam-host-ctrl-toggle"
                title="Все участники управляют музыкой"
                hint={
                  controlMode === "anyone"
                    ? "Участники могут переключать треки и ставить на паузу"
                    : "Только вы управляете воспроизведением, другие только добавляют музыку в очередь"
                }
                checked={controlMode === "anyone"}
                onChange={(checked) => jamEngine.setControlMode(checked ? "anyone" : "host_only")}
              />
            </div>
          )}

          {/* Footer Actions */}
          <div className="dialog-actions jam-dialog-actions">
            <button
              type="button"
              className="btn ghost"
              onClick={() => setOpen(false)}
            >
              Свернуть
            </button>
            <button
              type="button"
              className="btn danger"
              onClick={() => jamEngine.leaveOrEndSession()}
            >
              {isHost ? "Завершить сессию" : "Выйти из сессии"}
            </button>
          </div>
        </div>
      )}
    </Dialog>
  );
}
