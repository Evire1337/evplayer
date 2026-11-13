import { useEffect, useState } from "react";
import { toast } from "@/ui/toasts";
import { Pill } from "./parts";

// Settings → Services: the user's own proxy (src-tauri/src/userproxy.rs). For places where
// the services cannot be reached directly: every connection of the app goes through it.
export default function ProxyCard() {
  const api = window.evplayer?.proxy;
  const [saved, setSaved] = useState("");
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    void api?.get().then((v) => {
      setSaved(v);
      setTyped(v);
    });
  }, [api]);
  if (!api) return null;

  const save = async () => {
    setBusy(true);
    try {
      await api.set(typed.trim());
      const now = await api.get();
      setSaved(now);
      setTyped(now);
      toast(now ? "Прокси сохранён. Он заработает после перезапуска EvPlayer" : "Прокси убран. Это заработает после перезапуска EvPlayer");
    } catch (e) {
      toast(typeof e === "string" ? e : e instanceof Error ? e.message : "Не удалось сохранить", "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="svc-card">
      <div className="svc-head">
        <span className="svc-head-text">
          <strong>Свой прокси</strong>
          <span className="muted small">Если сервисы у вас не открываются без VPN, можно пустить через прокси все соединения EvPlayer: и поиск, и сами песни, и скачивание. Подойдёт любой HTTP-прокси, например порт «HTTP» или «mixed» в Clash, v2rayN или Hiddify, которые уже работают у вас на компьютере. Остальные программы это не затрагивает.</span>
        </span>
      </div>
      <div className="svc-actions">
        <input value={typed} onChange={(e) => setTyped(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void save()} placeholder="Например 127.0.0.1:7890. Пусто: без прокси" spellCheck={false} aria-label="Адрес прокси" />
        <button type="button" className="btn" disabled={busy || typed.trim() === saved} onClick={() => void save()}>Сохранить</button>
        <Pill tone={saved ? "ok" : "off"}>{saved ? "Включён после перезапуска" : "Не задан"}</Pill>
      </div>
    </section>
  );
}
