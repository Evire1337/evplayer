import { useEffect, useState } from "react";
import { useSettings } from "@/settings/store";
import { IconChart } from "@/ui/components/Icons";
import { toast } from "@/ui/toasts";
import { Probe, type Sample } from "@/diagnostics/probe";
import { advise, mb, report } from "@/diagnostics/advice";
import { isPhone } from "@/platform/device";
import type { ProcessInfo } from "@/platform/api";
import { Card, Pill, Row, TabHeader } from "./parts";

const EVERY_MS = 2000;

const KINDS: Record<string, string> = {
  renderer: "страница (окно или мини-плеер)",
  gpu: "графика",
  browser: "менеджер",
  utility: "служебный (сеть, звук, хранилище)",
  network: "сеть",
};

/** the browser engine the interface runs in: WebView2 on Windows, WebKit on Linux */
function engineOf(name: string): string | undefined {
  if (/msedgewebview2/i.test(name)) return "WebView2";
  if (/^WebKit/i.test(name)) return "WebKit";
  return undefined;
}

/** a process's name as people know it */
export function processName(name: string, kind?: string): string {
  const engine = engineOf(name);
  if (kind && KINDS[kind]) return engine ? `${engine}: ${KINDS[kind]}` : KINDS[kind];
  return engine ?? (/evplayer/i.test(name) ? "EvPlayer" : name);
}

/** what the total counts, in the words of the system it runs on */
function totalHint(processes: ProcessInfo[]): string {
  if (!processes.length) return "Только в приложении";
  if (isPhone) return "Только сам EvPlayer: интерфейс работает в WebView Android, его память приложению не видна";
  const count = `${processes.length} процесс(ов) приложения`;
  return processes.some((p) => engineOf(p.name) === "WebView2") ? `${count}, как «Память» в диспетчере задач` : `${count}, своя память без общих библиотек`;
}

/** what the list of processes shows */
function processesHint(processes: ProcessInfo[]): string {
  if (isPhone) return "Интерфейс работает в WebView Android, в процессах самой системы: здесь виден только EvPlayer";
  if (processes.some((p) => engineOf(p.name) === "WebKit")) return "WebKit — встроенный браузер, в котором работает интерфейс";
  return "WebView2 — встроенный браузер, в котором работает интерфейс";
}

// What the app takes right now and what to do about it. Measures only while this tab is open.
export default function DiagnosticsTab() {
  const [sample, setSample] = useState<Sample | null>(null);
  const settings = useSettings();

  useEffect(() => {
    const probe = new Probe();
    probe.start();
    let live = true;
    const read = () => void probe.sample().then((s) => live && setSample(s));
    read();
    const timer = setInterval(read, EVERY_MS);
    return () => {
      live = false;
      clearInterval(timer);
      probe.stop();
    };
  }, []);

  const advice = sample ? advise(sample, settings, { phone: isPhone }) : [];
  const total = sample?.processes.reduce((n, p) => n + p.memory, 0) ?? 0;
  const cpu = sample?.processes.reduce((n, p) => n + p.cpu, 0) ?? 0;

  const copy = () => {
    if (!sample) return;
    void navigator.clipboard.writeText(report(sample, advice)).then(() => toast("Отчёт скопирован: его можно прислать разработчику"));
  };

  return (
    <>
      <TabHeader icon={<IconChart size={22} />} title="Диагностика" description="Сколько памяти и процессора берёт приложение, что из этого стоит дороже всего и что с этим сделать. Замеры идут только пока открыт этот раздел." hue={200} />
      <Card title="Сейчас" actions={<button type="button" className="btn ghost" disabled={!sample} onClick={copy}>Скопировать отчёт</button>}>
        {!sample ? (
          <p className="muted small">Замеряю…</p>
        ) : (
          <>
            <Row title="Память всего" hint={totalHint(sample.processes)}>{total ? mb(total) : "—"}</Row>
            <Row title="Процессор" hint="Доля всего процессора, как в диспетчере задач">{sample.processes.length ? `${cpu.toFixed(1)}%` : "—"}</Row>
            <Row title="Память страницы" hint="Данные интерфейса: библиотека, очередь, картинки">{sample.heap ? mb(sample.heap.used) : "—"}</Row>
            <Row title="Главный поток" hint={`Таймеры опаздывают на ${Math.round(sample.lag)} мс; элементов на странице: ${sample.domNodes}`}>{`${Math.round(sample.busy * 100)}% занят`}</Row>
            <Row title="Живой фон" hint={settings.animatedBackdrop ? `${sample.backdrop.msPerFrame.toFixed(2)} мс на кадр` : "Выключен"}>{`${Math.round(sample.backdrop.fps)} кадр/с`}</Row>
            <Row title="Обработка звука" hint="Эквалайзер, эффекты, кроссфейд">{sample.soundGraph ? "включена" : "выключена"}</Row>
          </>
        )}
      </Card>
      <Card title="Советы">
        {sample && advice.length === 0 && <p className="muted small">Всё в порядке: ничего лишнего не тратится.</p>}
        {advice.map((a) => (
          <Row key={a.id} title={a.title} hint={a.detail} icon={<Pill tone={a.level === "high" ? "warn" : a.level === "medium" ? "info" : "off"}>{a.level === "high" ? "важно" : a.level === "medium" ? "стоит" : "к сведению"}</Pill>}>
            {a.fix && <button type="button" className="btn primary" onClick={() => settings.set(a.fix!.patch)}>{a.fix.label}</button>}
          </Row>
        ))}
      </Card>
      {sample && sample.processes.length > 0 && (
        <Card title="Процессы" hint={processesHint(sample.processes)}>
          {sample.processes.map((p) => (
            <Row key={p.pid} title={processName(p.name, p.kind)} hint={`#${p.pid}`}>{`${mb(p.memory)} · ${p.cpu.toFixed(1)}%`}</Row>
          ))}
        </Card>
      )}
      {sample && (
        <Card title="Данные">
          <Row title="Библиотека" hint={`${sample.counts.playlists} плейлистов, ${sample.counts.playlistTracks} треков в них`}>{`${sample.counts.likes} лайков`}</Row>
          <Row title="История и очередь" hint={`Офлайн: ${sample.counts.offline}`}>{`${sample.counts.history} / ${sample.counts.queue}`}</Row>
          {sample.storage.slice(0, 5).map((x) => (
            <Row key={x.key} title={x.key} hint="Хранится на этом устройстве">{mb(x.bytes)}</Row>
          ))}
        </Card>
      )}
    </>
  );
}
