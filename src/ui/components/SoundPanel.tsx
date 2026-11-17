import { clampPitch, clampSpeed, effectivePitch, MAX_PITCH, MAX_SPEED, MIN_SPEED, SPEED_PRESETS, useEffects } from "@/player/effects";
import { useSettings } from "@/settings/store";
import { Stepper, Toggle } from "./Controls";

export default function SoundPanel() {
  const fx = useEffects();
  const set = useEffects.setState;
  const pitch = effectivePitch(fx);
  const vocalCut = useSettings((s) => s.vocalCut);
  const setSetting = useSettings((s) => s.set);
  return (
    <div className="sound-panel">
      <Toggle id="fx-vocalcut" title="Без вокала" hint="Для всех песен сразу: убирает голос, который стоит по центру записи. Приём простой, на некоторых песнях остаётся след голоса" checked={vocalCut > 0} onChange={(on) => setSetting({ vocalCut: on ? 1 : 0 })} />
      {vocalCut > 0 && <Stepper id="fx-vocalcut-amount" label="Насколько убирать" value={Math.round(vocalCut * 100)} display={`${Math.round(vocalCut * 100)}%`} min={10} max={100} step={5} onChange={(v) => setSetting({ vocalCut: Math.min(1, Math.max(0.1, v / 100)) })} />}

      <div className="preset-row" role="group" aria-label="Быстрая скорость">
        {SPEED_PRESETS.map((p) => (
          <button key={p} type="button" className={`preset${Math.abs(fx.speed - p) < 0.001 ? " is-on" : ""}`} onClick={() => set({ speed: p })}>
            ×{p === 1 ? "1.0" : String(p)}
          </button>
        ))}
      </div>

      <Stepper id="fx-speed" label="Скорость" value={fx.speed} display={`×${fx.speed.toFixed(2)}`} min={MIN_SPEED} max={MAX_SPEED} step={0.01} onChange={(v) => set({ speed: clampSpeed(v) })} />

      <Stepper
        id="fx-pitch"
        label={fx.linked ? "Тон (идёт за скоростью)" : "Тон"}
        value={Math.round(pitch * 10) / 10}
        display={`${pitch > 0 ? "+" : ""}${pitch.toFixed(1)} пт`}
        min={-MAX_PITCH}
        max={MAX_PITCH}
        step={0.1}
        disabled={fx.linked}
        onChange={(v) => set({ pitch: clampPitch(v) })}
      />

      <Toggle id="fx-linked" title="Тон вместе со скоростью" hint="Как пластинка: быстрее — выше, медленнее — ниже" checked={fx.linked} onChange={(linked) => set({ linked })} />
      <Toggle id="fx-keep" title="Оставлять для следующих треков" hint="Иначе всё сбросится, когда начнётся новый трек" checked={fx.keep} onChange={(keep) => set({ keep })} />
      <Toggle id="fx-reverb" title="Реверберация" hint="Объём и пространство, как в зале" checked={fx.reverb} onChange={(reverb) => set({ reverb })} />
      {fx.reverb && (
        <Stepper id="fx-reverb-amount" label="Сила реверберации" value={Math.round(fx.reverbAmount * 100)} display={`${Math.round(fx.reverbAmount * 100)}%`} min={0} max={100} step={1} onChange={(v) => set({ reverbAmount: Math.min(1, Math.max(0, v / 100)) })} />
      )}

      <button type="button" className="reset-btn" onClick={() => set({ speed: 1, pitch: 0, linked: false, reverb: false })}>
        Сбросить
      </button>
    </div>
  );
}
