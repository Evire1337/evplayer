import { useEffect, useRef, useState } from "react";
import { useSettings } from "@/settings/store";
import { MAX_LEVEL, MIX_KEYS, isFlat, effectiveLevels, type MixKey } from "@/audio/soundMixer";
import { peakOf } from "@/audio/meters";
import { player } from "@/player/engine";
import { currentTrack, usePlayer } from "@/player/store";
import { effectivePitch, useEffects } from "@/player/effects";
import Artwork from "@/ui/components/Artwork";
import SoundPanel from "@/ui/components/SoundPanel";
import { IconBass, IconMic, IconMusic, IconRefresh, IconSliders, IconWidth } from "@/ui/components/Icons";

// "Дорожки": the sound mixer for every song (audio/soundMixer.ts), laid out like a studio:
// a lane for each part of the song with its own mute, solo and level, and next to it what that
// part carries right now. It is a setting, not an effect: it stays when the next song starts.
// Speed, pitch, reverb and the equalizer work on what comes out of it.

const LANES: Array<{ key: MixKey; title: string; hint: string; color: string; icon: (p: { size?: number }) => React.ReactNode }> = [
  { key: "voice", title: "Голос", hint: "середина записи, средние частоты", color: "#6ff0c4", icon: IconMic },
  { key: "bass", title: "Бас", hint: "всё ниже 150 Гц", color: "#5fb4ff", icon: IconBass },
  { key: "width", title: "Инструменты", hint: "стороны: чем они шире, тем больше", color: "#c7a0ff", icon: IconWidth },
];

type Faders = Record<MixKey, number>;

const PRESETS: Array<{ name: string; faders: Faders }> = [
  { name: "Без вокала", faders: { voice: 0, bass: 1, width: 1 } },
  { name: "Голос тише", faders: { voice: 0.4, bass: 1, width: 1 } },
  { name: "Голос громче", faders: { voice: 1.4, bass: 1, width: 1 } },
  { name: "Больше баса", faders: { voice: 1, bass: 1.5, width: 1 } },
  { name: "Моно", faders: { voice: 1, bass: 1, width: 0 } },
];

const same = (a: Faders, b: Faders) => MIX_KEYS.every((k) => Math.abs(a[k] - b[k]) < 0.001);

/** how many moments of each part are kept and drawn (one every HISTORY_STEP_MS) */
const HISTORY = 360;
const HISTORY_STEP_MS = 40;

export default function StudioPage() {
  const voice = useSettings((s) => s.mixVoice);
  const bass = useSettings((s) => s.mixBass);
  const width = useSettings((s) => s.mixWidth);
  const muted = useSettings((s) => s.mixMuted);
  const solo = useSettings((s) => s.mixSolo);
  const set = useSettings((s) => s.set);
  const faders: Faders = { voice, bass, width };
  const flat = isFlat(effectiveLevels(faders, muted, solo));
  const status = usePlayer((s) => s.status);
  const track = usePlayer((s) => currentTrack(s));
  const fx = useEffects();
  const [tuning, setTuning] = useState(false);

  const toggleMute = (k: MixKey) => set({ mixMuted: muted.includes(k) ? muted.filter((m) => m !== k) : [...muted, k] });
  const toggleSolo = (k: MixKey) => set({ mixSolo: solo === k ? null : k });
  const reset = () => set({ mixVoice: 1, mixBass: 1, mixWidth: 1, mixMuted: [], mixSolo: null });
  const apply = (f: Faders) => set({ mixVoice: f.voice, mixBass: f.bass, mixWidth: f.width, mixMuted: [], mixSolo: null });
  const level = (k: MixKey, v: number) => set({ [k === "voice" ? "mixVoice" : k === "bass" ? "mixBass" : "mixWidth"]: v });

  const canvases = useRef<Record<MixKey, HTMLCanvasElement | null>>({ voice: null, bass: null, width: null });
  const live = status === "playing";

  // what each part carries: read while the song plays and the page is open
  useEffect(() => {
    const history: Record<MixKey, Float32Array> = { voice: new Float32Array(HISTORY), bass: new Float32Array(HISTORY), width: new Float32Array(HISTORY) };
    const buffer = new Float32Array(1024);
    let meters = player.openMeters();
    const draw = () => {
      for (const lane of LANES) {
        const cv = canvases.current[lane.key];
        if (!cv) continue;
        const w = cv.clientWidth;
        const h = cv.clientHeight;
        if (!w || !h) continue;
        const ratio = window.devicePixelRatio || 1;
        if (cv.width !== Math.round(w * ratio) || cv.height !== Math.round(h * ratio)) {
          cv.width = Math.round(w * ratio);
          cv.height = Math.round(h * ratio);
        }
        const g = cv.getContext("2d");
        if (!g) continue;
        g.setTransform(ratio, 0, 0, ratio, 0, 0);
        g.clearRect(0, 0, w, h);
        g.fillStyle = "rgba(5, 16, 18, 0.72)";
        const step = w / HISTORY;
        const data = history[lane.key];
        for (let i = 0; i < HISTORY; i++) {
          // the loudest sound reaches about half of the lane; quieter ones are lifted so they show
          const v = Math.min(1, Math.sqrt(data[i]) * 1.1);
          const bar = Math.max(1, v * (h - 8));
          g.fillRect(i * step, (h - bar) / 2, Math.max(1, step - 0.6), bar);
        }
      }
    };
    const timer = setInterval(() => {
      meters ??= player.openMeters();
      if (meters && usePlayer.getState().status === "playing") {
        for (const lane of LANES) {
          const data = history[lane.key];
          data.copyWithin(0, 1);
          data[HISTORY - 1] = peakOf(meters.analysers[lane.key], buffer);
        }
      }
      draw();
    }, HISTORY_STEP_MS);
    return () => {
      clearInterval(timer);
      player.closeMeters();
    };
  }, []);

  const pitch = effectivePitch(fx);

  return (
    <div className="page studio-page">
      <header className="studio-top">
        <span className="studio-cover">{track?.artwork ? <Artwork src={track.artwork} size="row" /> : <IconMusic size={26} />}</span>
        <div className="studio-title">
          <span className="eyebrow">Дорожки · звук всех песен</span>
          <h1 className="studio-song">{track ? track.title : "Ничего не играет"}</h1>
          <span className="muted small">{track ? track.artists.map((a) => a.name).join(", ") : "Включите песню, и здесь будет видно, что несёт каждая дорожка"}</span>
        </div>
        <div className="studio-tools">
          <div className="studio-drop">
            <button type="button" className={`studio-pill${tuning ? " is-open" : ""}`} aria-expanded={tuning} onClick={() => setTuning((v) => !v)}>
              <IconSliders size={16} />
              <span>×{fx.speed.toFixed(2)}</span>
              <span className="studio-pill-sep" />
              <span>{pitch > 0 ? "+" : ""}{pitch.toFixed(1)} пт</span>
            </button>
            {tuning && (
              <>
                <button type="button" className="studio-drop-shade" aria-label="Закрыть" onClick={() => setTuning(false)} />
                <div className="studio-drop-panel"><SoundPanel /></div>
              </>
            )}
          </div>
          <button type="button" className="studio-pill" disabled={flat} onClick={reset}>
            <IconRefresh size={15} /> Сбросить
          </button>
        </div>
      </header>

      <div className="chip-row studio-presets" role="group" aria-label="Быстрые настройки">
        {PRESETS.map((p) => {
          const on = same(faders, p.faders) && !muted.length && !solo;
          return (
            <button key={p.name} type="button" className={`chip${on ? " is-on" : ""}`} aria-pressed={on} onClick={() => apply(p.faders)}>
              {p.name}
            </button>
          );
        })}
      </div>

      <section className="studio-board" aria-label="Дорожки">
        <div className="studio-ruler" aria-hidden="true">
          <span />
          <span className="studio-ruler-line"><i>−14 с</i><i>−7 с</i><i>сейчас</i></span>
        </div>
        {LANES.map(({ key, title, hint, color, icon: Icon }) => {
          const value = faders[key];
          const pct = Math.round(value * 100);
          const isMuted = muted.includes(key);
          const isSolo = solo === key;
          const dim = isMuted || (solo !== null && !isSolo);
          return (
            <div key={key} className={`studio-lane${dim ? " is-dim" : ""}`} style={{ "--lane": color } as React.CSSProperties}>
              <div className="studio-lane-label">
                <div className="studio-lane-row">
                  <button type="button" className={`studio-ms is-mute${isMuted ? " is-on" : ""}`} aria-pressed={isMuted} aria-label={`Без звука: ${title}`} data-tip="Выключить" onClick={() => toggleMute(key)}>M</button>
                  <button type="button" className={`studio-ms is-solo${isSolo ? " is-on" : ""}`} aria-pressed={isSolo} aria-label={`Только это: ${title}`} data-tip="Только эта дорожка" onClick={() => toggleSolo(key)}>S</button>
                  <span className="studio-lane-icon"><Icon size={17} /></span>
                  <strong className="studio-lane-name">{title}</strong>
                  <output htmlFor={`mix-${key}`} className="studio-lane-value">{pct}%</output>
                </div>
                <input
                  id={`mix-${key}`}
                  type="range"
                  className="dial"
                  aria-label={`${title}: громкость`}
                  min={0}
                  max={MAX_LEVEL * 100}
                  step={5}
                  value={pct}
                  onChange={(e) => level(key, Number(e.target.value) / 100)}
                  onDoubleClick={() => level(key, 1)}
                  style={{ "--fill": `${(pct / (MAX_LEVEL * 100)) * 100}%` } as React.CSSProperties}
                />
                <span className="muted small studio-lane-hint">{hint}</span>
              </div>
              <div className="studio-lane-wave">
                <canvas ref={(el) => { canvases.current[key] = el; }} aria-hidden="true" />
                {!live && <span className="studio-lane-idle">{track ? "На паузе" : "Тишина"}</span>}
              </div>
            </div>
          );
        })}
      </section>

      <p className="muted small studio-note">
        Это лёгкий способ: песня не раскладывается на отдельные инструменты, а делится по тому, где в записи стоит звук. Поэтому голос с широкой реверберацией может оставить след, а на моно-записях голос не отделить совсем. Темп, тон, реверберация и эквалайзер работают на результате как обычно. Двойной клик по ползунку возвращает 100%.
      </p>
    </div>
  );
}
