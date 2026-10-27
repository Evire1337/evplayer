import { useEffect, useState } from "react";
import { useSettings, type OverlayCorner } from "@/settings/store";
import { binding, prettyAccelerator, SHORTCUTS } from "@/settings/shortcuts";
import { IconPip } from "@/ui/components/Icons";
import { Card, Row, SwitchRow, TabHeader } from "./parts";

const CORNERS: Array<{ id: OverlayCorner; label: string }> = [
  { id: "top-left", label: "Слева сверху" },
  { id: "top-right", label: "Справа сверху" },
  { id: "bottom-left", label: "Слева снизу" },
  { id: "bottom-right", label: "Справа снизу" },
];

export default function OverlayTab() {
  const o = useSettings();
  const [visible, setVisible] = useState(false);
  useEffect(() => window.evplayer?.overlay.onVisible(setVisible), []);
  const key = (id: string) => prettyAccelerator(binding(o.shortcuts, SHORTCUTS.find((s) => s.id === id)!)).join(" + ") || "не задано";
  return (
    <>
      <TabHeader icon={<IconPip size={22} />} hue={190} title="Мини-плеер" description="Маленькое окно поверх всех программ: обложка, название, строка текста и кнопки." />
      <Card>
        <div className="ov-hero">
          <div className="ov-sample" aria-hidden="true">
            <span className="ov-sample-art" />
            <span className="ov-sample-lines"><span /><span /><span className="short" /></span>
          </div>
          <div className="ov-hero-text">
            <span className="set-row-title">{visible ? "Мини-плеер открыт" : "Мини-плеер спрятан"}</span>
            <span className="set-row-hint">Перетаскивайте его за любое место. Крестик в углу прячет его обратно.</span>
            <button type="button" className="btn primary" disabled={!o.overlayEnabled} onClick={() => window.evplayer?.overlay.toggle()}>
              {visible ? "Спрятать" : "Показать сейчас"}
            </button>
          </div>
        </div>
      </Card>
      <Card title="Поведение">
        <SwitchRow title="Мини-плеер включён" hint="Если выключить, его нельзя будет открыть ни клавишей, ни из трея" checked={o.overlayEnabled} onChange={(v) => o.set({ overlayEnabled: v })} />
        <SwitchRow title="Поверх всех окон" hint="Не прячется за браузером и играми в окне" checked={o.overlayOnTop} onChange={(v) => o.set({ overlayOnTop: v })} disabled={!o.overlayEnabled} />
        <SwitchRow title="Показывать при сворачивании" hint="Свернули EvPlayer — появился мини-плеер; развернули — исчез" checked={o.overlayOnMinimize} onChange={(v) => o.set({ overlayOnMinimize: v })} disabled={!o.overlayEnabled} />
        <SwitchRow title="Строка текста песни" hint="Вместо исполнителя показывать строку, которую сейчас поют" checked={o.overlayShowLyrics} onChange={(v) => o.set({ overlayShowLyrics: v })} disabled={!o.overlayEnabled} />
      </Card>
      <Card title="Где появляется" hint="Если вы перетащили мини-плеер, он остаётся там, пока не выберете угол заново">
        <div className="corner-picker" role="radiogroup" aria-label="Угол экрана">
          {CORNERS.map((c) => (
            <button key={c.id} type="button" role="radio" aria-checked={o.overlayCorner === c.id} className={`corner corner-${c.id}${o.overlayCorner === c.id ? " is-on" : ""}`} onClick={() => o.set({ overlayCorner: c.id })} data-tip={c.label}>
              <span />
            </button>
          ))}
        </div>
      </Card>
      <Card title="Клавиши">
        <Row title="В окне EvPlayer" hint={key("app.overlay")} />
        <Row title="Из любой программы" hint={key("global.overlay")} />
        <p className="muted small">Поменять — во вкладке «Клавиши». Ещё мини-плеер открывается из меню значка в трее.</p>
      </Card>
    </>
  );
}
