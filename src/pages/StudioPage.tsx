import { useEffect, useMemo, useRef, useState } from "react";
import { useSettings } from "@/settings/store";
import { MAX_LEVEL, MIX_KEYS, isFlat, effectiveLevels, type MixKey } from "@/audio/soundMixer";
import { peakOf } from "@/audio/meters";
import { player } from "@/player/engine";
import { currentTrack, usePlayer } from "@/player/store";
import { effectivePitch, useEffects } from "@/player/effects";
import { keyOf } from "@/stems/control";
import { MAX_STEM_LEVEL, PARTS, PART_HINT, PART_TITLE, STEM_PRESETS, samePreset, type Part } from "@/stems/model";
import { useStems } from "@/stems/store";
import SoundPanel from "@/ui/components/SoundPanel";
import { IconBass, IconMic, IconRefresh, IconSliders, IconWidth } from "@/ui/components/Icons";
import { Card, Pill, SwitchRow, TabHeader } from "./settings/parts";
import { NameDialog } from "./settings/EqualizerSection";
import { addPreset, decodeStemPreset, encodeStemPreset, movePreset, newPreset, overwritePreset, removePreset, renamePreset, samePresetLevels } from "@/stems/presets";
import { IconMore } from "@/ui/components/Icons";
import { openMenu } from "@/ui/menu";
import { toast } from "@/ui/toasts";
import { useReorder } from "@/ui/useReorder";

// "Дорожки": the studio. With the switch on, a song is split into vocals, drums, bass and the
// rest by a neural network on this computer (src-tauri/src/stems.rs), and each track has its
// own mute, solo and level next to the track's whole wave. With the switch off nothing is
// loaded, and the same page works as a light mixer of the voice, the bass and the width of any
// song (audio/soundMixer.ts). Speed, pitch, reverb and the equalizer work on either.

const PART_HUE: Record<Part, number> = { vocals: 160, drums: 28, bass: 205, other: 275 };
const LIGHT_LANES: Array<{ key: MixKey; title: string; hint: string; icon: (p: { size?: number }) => React.ReactNode; hue: number }> = [
  { key: "voice", title: "Голос", hint: "середина записи, средние частоты", icon: IconMic, hue: 160 },
  { key: "bass", title: "Бас", hint: "всё ниже 150 Гц", icon: IconBass, hue: 205 },
  { key: "width", title: "Инструменты", hint: "стороны записи", icon: IconWidth, hue: 275 },
];
const LIGHT_PRESETS: Array<{ name: string; faders: Record<MixKey, number> }> = [
  { name: "Без вокала", faders: { voice: 0, bass: 1, width: 1 } },
  { name: "Голос тише", faders: { voice: 0.4, bass: 1, width: 1 } },
  { name: "Голос громче", faders: { voice: 1.4, bass: 1, width: 1 } },
  { name: "Больше баса", faders: { voice: 1, bass: 1.5, width: 1 } },
  { name: "Моно", faders: { voice: 1, bass: 1, width: 0 } },
];
const CACHE_CHOICES = [500, 1000, 2000, 5000];

const hsl = (hue: number, l = 62) => `hsl(${hue} 75% ${l}%)`;
const clock = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;
const megabytes = (bytes: number) => (bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} ГБ` : `${Math.round(bytes / 1e6)} МБ`);

/** how many moments of the live waves are kept and drawn (one every LIVE_STEP_MS) */
const LIVE = 320;
const LIVE_STEP_MS = 45;

function fit(cv: HTMLCanvasElement): CanvasRenderingContext2D | null {
  const w = cv.clientWidth;
  const h = cv.clientHeight;
  if (!w || !h) return null;
  const ratio = window.devicePixelRatio || 1;
  if (cv.width !== Math.round(w * ratio) || cv.height !== Math.round(h * ratio)) {
    cv.width = Math.round(w * ratio);
    cv.height = Math.round(h * ratio);
  }
  const g = cv.getContext("2d");
  g?.setTransform(ratio, 0, 0, ratio, 0, 0);
  return g;
}

/** the whole wave of a split track, with the played part lit and a line where the song is now */
function PartWave({ peaks, hue, dim }: { peaks: number[] | null; hue: number; dim: boolean }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const duration = usePlayer((s) => s.durationMs);

  useEffect(() => {
    const draw = () => {
      const cv = ref.current;
      if (!cv) return;
      const g = fit(cv);
      if (!g) return;
      const w = cv.clientWidth;
      const h = cv.clientHeight;
      g.clearRect(0, 0, w, h);
      const here = duration > 0 ? Math.min(1, usePlayer.getState().positionMs / duration) : 0;
      const mid = h / 2;
      // a faint centre line, like the equalizer's zero line
      g.fillStyle = "rgba(238, 247, 244, 0.1)";
      g.fillRect(0, mid, w, 1);
      if (peaks?.length) {
        const bars = Math.min(peaks.length, Math.floor(w / 3));
        const step = w / bars;
        for (let i = 0; i < bars; i++) {
          const v = peaks[Math.min(peaks.length - 1, Math.floor((i / bars) * peaks.length))] / 255;
          const bar = Math.max(1.5, Math.sqrt(v) * (h - 10));
          g.fillStyle = i / bars <= here ? hsl(hue, 66) : "rgba(238, 247, 244, 0.3)";
          g.fillRect(i * step, mid - bar / 2, Math.max(1.2, step - 1), bar);
        }
      }
      g.fillStyle = "rgba(238, 247, 244, 0.9)";
      g.fillRect(here * w - 0.5, 0, 1.5, h);
    };
    draw();
    const off = usePlayer.subscribe((s, prev) => {
      if (s.positionMs !== prev.positionMs) draw();
    });
    window.addEventListener("resize", draw);
    return () => {
      off();
      window.removeEventListener("resize", draw);
    };
  }, [peaks, hue, duration]);

  return (
    <canvas
      ref={ref}
      className={`studio-wave${dim ? " is-dim" : ""}`}
      aria-hidden="true"
      onClick={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        if (duration > 0) player.seek(((e.clientX - r.left) / r.width) * duration);
      }}
    />
  );
}

/** the live wave of a part of any song: what it carries over the last seconds */
function LiveWave({ lane, history, dim }: { lane: MixKey; history: React.MutableRefObject<Record<MixKey, Float32Array>>; dim: boolean }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const timer = setInterval(() => {
      const cv = ref.current;
      if (!cv) return;
      const g = fit(cv);
      if (!g) return;
      const w = cv.clientWidth;
      const h = cv.clientHeight;
      g.clearRect(0, 0, w, h);
      g.fillStyle = "rgba(238, 247, 244, 0.1)";
      g.fillRect(0, h / 2, w, 1);
      const data = history.current[lane];
      const step = w / LIVE;
      for (let i = 0; i < LIVE; i++) {
        const bar = Math.max(1.5, Math.sqrt(data[i]) * 1.1 * (h - 10));
        g.fillStyle = `rgba(238, 247, 244, ${0.25 + 0.5 * (i / LIVE)})`;
        g.fillRect(i * step, (h - bar) / 2, Math.max(1.2, step - 0.8), bar);
      }
    }, LIVE_STEP_MS);
    return () => clearInterval(timer);
  }, [lane, history]);
  return <canvas ref={ref} className={`studio-wave${dim ? " is-dim" : ""}`} aria-hidden="true" />;
}

/** the quick settings of the four tracks: ready-made ones, and the user's own, like the equalizer's */
function StemPresets() {
  const s = useSettings();
  const set = useSettings((x) => x.set);
  const list = s.stemPresets;
  const reorder = useReorder((from, to) => set({ stemPresets: movePreset(list, from, to) }), "x");
  const [dialog, setDialog] = useState<{ kind: "save" | "rename" | "import"; id?: string; initial: string } | null>(null);
  const apply = (faders: typeof s.stemFaders, muted: Part[]) => set({ stemFaders: { ...faders }, stemMuted: [...muted], stemSolo: null });
  return (
    <>
      <div className="chip-row studio-presets" role="group" aria-label="Быстрые настройки">
        {STEM_PRESETS.map((p) => {
          const on = samePreset(p, s.stemFaders, s.stemMuted, s.stemSolo);
          return (
            <button key={p.name} type="button" className={`chip${on ? " is-on" : ""}`} aria-pressed={on} onClick={() => apply(p.faders, p.muted)}>
              {p.name}
            </button>
          );
        })}
      </div>
      <div className="chip-row studio-presets" role="group" aria-label="Свои пресеты">
        <span className="eyebrow studio-eyebrow">Свои</span>
        {list.map((p, i) => {
          const on = s.stemSolo === null && samePresetLevels(p, s.stemFaders, s.stemMuted);
          return (
            <span key={p.id} className={`chip is-draggable${on ? " is-on" : ""}${reorder.hint(i) ? ` drop-${reorder.hint(i)}` : ""}`} {...reorder.rowProps(i)}>
              <button type="button" onClick={() => apply(p.faders, p.muted)}>{p.name}</button>
              <button
                type="button"
                className="chip-more"
                aria-label={`Ещё: ${p.name}`}
                onClick={(e) => {
                  const r = e.currentTarget.getBoundingClientRect();
                  openMenu({ clientX: r.left, clientY: r.bottom + 4 }, [
                    { label: "Сохранить текущие настройки сюда", onSelect: () => { set({ stemPresets: overwritePreset(list, p.id, s.stemFaders, s.stemMuted) }); toast(`«${p.name}» обновлён`); } },
                    { label: "Переименовать", onSelect: () => setDialog({ kind: "rename", id: p.id, initial: p.name }) },
                    { label: "Скопировать код пресета", onSelect: () => void navigator.clipboard.writeText(encodeStemPreset(p)).then(() => toast("Код скопирован — отправьте его другу")) },
                    { label: "Удалить", danger: true, onSelect: () => set({ stemPresets: removePreset(list, p.id) }) },
                  ]);
                }}
              >
                <IconMore size={14} />
              </button>
            </span>
          );
        })}
        <button type="button" className="chip is-add" onClick={() => setDialog({ kind: "save", initial: "" })}>+ Сохранить текущие</button>
        <button type="button" className="chip is-add" onClick={() => setDialog({ kind: "import", initial: "" })}>Вставить код</button>
      </div>
      {dialog && (
        <NameDialog
          open
          title={dialog.kind === "save" ? "Новый пресет" : dialog.kind === "rename" ? "Переименовать пресет" : "Пресет по коду"}
          initial={dialog.initial}
          multiline={dialog.kind === "import"}
          onClose={() => setDialog(null)}
          onSubmit={(value) => {
            if (dialog.kind === "save") {
              const created = newPreset(value, s.stemFaders, s.stemMuted);
              set({ stemPresets: addPreset(list, created) });
              toast(`Пресет «${created.name}» сохранён`);
            } else if (dialog.kind === "rename" && dialog.id) {
              set({ stemPresets: renamePreset(list, dialog.id, value) });
            } else {
              const created = decodeStemPreset(value);
              if (!created) return "Код повреждён — скопируйте его целиком";
              set({ stemPresets: addPreset(list, created) });
              toast(`Пресет «${created.name}» добавлен`);
            }
          }}
        />
      )}
    </>
  );
}

function Lane({ title, hint, hue, icon, value, max, muted, solo, dim, onValue, onMute, onSolo, children }: {
  title: string;
  hint: string;
  hue: number;
  icon?: React.ReactNode;
  value: number;
  max: number;
  muted: boolean;
  solo: boolean;
  dim: boolean;
  onValue: (v: number) => void;
  onMute: () => void;
  onSolo: () => void;
  children: React.ReactNode;
}) {
  const pct = Math.round(value * 100);
  return (
    <div className={`studio-lane${dim ? " is-dim" : ""}`}>
      <div className="studio-lane-label">
        <div className="studio-lane-row">
          <span className="band-dot" style={{ background: hsl(hue) }} />
          {icon}
          <strong className="studio-lane-name">{title}</strong>
          <output className="studio-lane-value">{pct}%</output>
        </div>
        <input
          type="range"
          className="dial"
          aria-label={`${title}: громкость`}
          min={0}
          max={max * 100}
          step={5}
          value={pct}
          onChange={(e) => onValue(Number(e.target.value) / 100)}
          onDoubleClick={() => onValue(1)}
          style={{ "--fill": `${(pct / (max * 100)) * 100}%` } as React.CSSProperties}
        />
        <div className="studio-lane-foot">
          <span className="muted small">{hint}</span>
          <span className="studio-ms-group">
            <button type="button" className={`studio-ms is-mute${muted ? " is-on" : ""}`} aria-pressed={muted} aria-label={`Без звука: ${title}`} data-tip="Выключить" onClick={onMute}>M</button>
            <button type="button" className={`studio-ms is-solo${solo ? " is-on" : ""}`} aria-pressed={solo} aria-label={`Только это: ${title}`} data-tip="Только эта дорожка" onClick={onSolo}>S</button>
          </span>
        </div>
      </div>
      <div className="studio-lane-wave">{children}</div>
    </div>
  );
}

export default function StudioPage() {
  const s = useSettings();
  const set = useSettings((x) => x.set);
  const phase = useStems((x) => x.phase);
  const modelShare = useStems((x) => x.modelShare);
  const error = useStems((x) => x.error);
  const cacheBytes = useStems((x) => x.cacheBytes);
  const jobs = useStems((x) => x.jobs);
  const engaged = useStems((x) => x.engaged);
  const status = usePlayer((x) => x.status);
  const track = usePlayer((x) => currentTrack(x));
  const fx = useEffects();
  const [tuning, setTuning] = useState(false);
  const pitch = effectivePitch(fx);

  const job = track ? jobs[keyOf(track.id) ?? ""] : undefined;
  const meta = job?.status === "ready" ? job.meta : undefined;
  const neural = s.stemsOn && phase === "ready";

  // ---- the light mixer (the switch is off) ----
  const light = { voice: s.mixVoice, bass: s.mixBass, width: s.mixWidth };
  const lightKeyed = (k: MixKey) => (k === "voice" ? "mixVoice" : k === "bass" ? "mixBass" : "mixWidth") as "mixVoice" | "mixBass" | "mixWidth";
  const lightFlat = isFlat(effectiveLevels(light, s.mixMuted, s.mixSolo));
  const live = useRef<Record<MixKey, Float32Array>>({ voice: new Float32Array(LIVE), bass: new Float32Array(LIVE), width: new Float32Array(LIVE) });

  useEffect(() => {
    if (neural) return;
    const buffer = new Float32Array(1024);
    let meters = player.openMeters();
    const timer = setInterval(() => {
      meters ??= player.openMeters();
      if (meters && usePlayer.getState().status === "playing") {
        for (const k of MIX_KEYS) {
          const data = live.current[k];
          data.copyWithin(0, 1);
          data[LIVE - 1] = peakOf(meters.analysers[k], buffer);
        }
      }
    }, LIVE_STEP_MS);
    return () => {
      clearInterval(timer);
      player.closeMeters();
    };
  }, [neural]);

  const total = useMemo(() => (meta ? meta.seconds : track ? track.durationMs / 1000 : 0), [meta, track]);
  const anySolo = s.stemSolo !== null;

  return (
    <div className="page studio-page">
      <TabHeader icon={<IconSliders size={26} />} title="Дорожки" description="Вокал, барабаны, бас и всё остальное: у каждой дорожки своя громкость, свой «выключить» и «только эта». Работает на любой песне, в том числе с реверберацией, скоростью и эквалайзером." hue={160} />

      <Card>
        <SwitchRow
          title="Точное разделение нейросетью"
          hint={s.stemsOn ? "Модель в памяти, песни из очереди разделяются в фоне. Выключите, и всё выгрузится." : "Выключено: ничего не загружено и не работает. Включите, и скачается модель (один раз, около 80 МБ)."}
          checked={s.stemsOn}
          onChange={(v) => set({ stemsOn: v })}
          icon={<IconSliders size={16} />}
        />
        {s.stemsOn && (
          <div className="studio-status">
            {phase === "download" && (
              <>
                <Pill tone="info">Скачиваю модель</Pill>
                <div className="studio-bar"><i style={{ width: `${Math.round(modelShare * 100)}%` }} /></div>
                <span className="muted small">{Math.round(modelShare * 100)}%</span>
              </>
            )}
            {phase === "loading" && <Pill tone="info">Загружаю модель в память…</Pill>}
            {phase === "ready" && <Pill tone="ok">Готово: модель в памяти</Pill>}
            {phase === "error" && <Pill tone="warn">{error ?? "Не получилось"}</Pill>}
          </div>
        )}
      </Card>

      <section className="studio-panel" aria-label="Дорожки">
        <header className="studio-top">
          <div className="studio-song">
            <strong>{track ? track.title : "Ничего не играет"}</strong>
            <span className="muted small">{track ? track.artists.map((a) => a.name).join(", ") : "Включите песню"}</span>
          </div>
          <div className="studio-drop">
            <button type="button" className={`studio-pill${tuning ? " is-open" : ""}`} aria-expanded={tuning} onClick={() => setTuning((v) => !v)}>
              <IconSliders size={15} />
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
          <button
            type="button"
            className="studio-pill"
            disabled={neural ? !s.stemMuted.length && s.stemSolo === null && PARTS.every((p) => s.stemFaders[p] === 1) : lightFlat}
            onClick={() => (neural ? set({ stemFaders: { vocals: 1, drums: 1, bass: 1, other: 1 }, stemMuted: [], stemSolo: null }) : set({ mixVoice: 1, mixBass: 1, mixWidth: 1, mixMuted: [], mixSolo: null }))}
          >
            <IconRefresh size={14} /> Сбросить
          </button>
        </header>

        {neural ? (
          <StemPresets />
        ) : (
          <div className="chip-row studio-presets" role="group" aria-label="Быстрые настройки">
            {LIGHT_PRESETS.map((p) => {
              const on = MIX_KEYS.every((k) => Math.abs(light[k] - p.faders[k]) < 0.001) && !s.mixMuted.length && !s.mixSolo;
              return (
                <button key={p.name} type="button" className={`chip${on ? " is-on" : ""}`} aria-pressed={on} onClick={() => set({ mixVoice: p.faders.voice, mixBass: p.faders.bass, mixWidth: p.faders.width, mixMuted: [], mixSolo: null })}>
                  {p.name}
                </button>
              );
            })}
          </div>
        )}

        {neural && job && job.status !== "ready" && (
          <div className="studio-job">
            <Pill tone={job.status === "error" ? "warn" : "info"}>
              {job.status === "queued" ? "В очереди на разделение" : job.status === "decoding" ? "Читаю звук песни…" : job.status === "separating" ? `Разделяю: ${Math.round(job.share * 100)}%` : (job.error ?? "Не вышло")}
            </Pill>
            {(job.status === "separating" || job.status === "decoding") && <div className="studio-bar"><i style={{ width: `${Math.round(job.share * 100)}%` }} /></div>}
            <span className="muted small">Пока идёт разделение, песня играет как обычно.</span>
          </div>
        )}

        <div className="studio-ruler" aria-hidden="true">
          <span />
          <span className="studio-ruler-line">
            {[0, 0.25, 0.5, 0.75, 1].map((p) => (
              <i key={p}>{total > 0 ? clock(total * p) : ""}</i>
            ))}
          </span>
        </div>

        {neural
          ? PARTS.map((p, i) => {
              const muted = s.stemMuted.includes(p);
              const isSolo = s.stemSolo === p;
              return (
                <Lane
                  key={p}
                  title={PART_TITLE[p]}
                  hint={PART_HINT[p]}
                  hue={PART_HUE[p]}
                  value={s.stemFaders[p]}
                  max={MAX_STEM_LEVEL}
                  muted={muted}
                  solo={isSolo}
                  dim={muted || (anySolo && !isSolo)}
                  onValue={(v) => set({ stemFaders: { ...s.stemFaders, [p]: v } })}
                  onMute={() => set({ stemMuted: muted ? s.stemMuted.filter((m) => m !== p) : [...s.stemMuted, p] })}
                  onSolo={() => set({ stemSolo: isSolo ? null : p })}
                >
                  <PartWave peaks={meta?.peaks[i] ?? null} hue={PART_HUE[p]} dim={muted || (anySolo && !isSolo)} />
                  {!meta && <span className="studio-lane-idle">{track ? "Волна появится после разделения" : "Тишина"}</span>}
                </Lane>
              );
            })
          : LIGHT_LANES.map(({ key, title, hint, icon: Icon, hue }) => {
              const muted = s.mixMuted.includes(key);
              const isSolo = s.mixSolo === key;
              const dim = muted || (s.mixSolo !== null && !isSolo);
              return (
                <Lane
                  key={key}
                  title={title}
                  hint={hint}
                  hue={hue}
                  icon={<Icon size={16} />}
                  value={light[key]}
                  max={MAX_LEVEL}
                  muted={muted}
                  solo={isSolo}
                  dim={dim}
                  onValue={(v) => set({ [lightKeyed(key)]: v })}
                  onMute={() => set({ mixMuted: muted ? s.mixMuted.filter((m) => m !== key) : [...s.mixMuted, key] })}
                  onSolo={() => set({ mixSolo: isSolo ? null : key })}
                >
                  <LiveWave lane={key} history={live} dim={dim} />
                  {status !== "playing" && <span className="studio-lane-idle">{track ? "На паузе" : "Тишина"}</span>}
                </Lane>
              );
            })}
      </section>

      {s.stemsOn && (
        <Card title="Разделённые песни" hint="Лежат на диске, чтобы второй раз открываться сразу. Когда переключатель выключен, они ничего не занимают в памяти.">
          <div className="studio-cache">
            <span className="muted">{megabytes(cacheBytes)} из {megabytes(s.stemsCacheMb * 1e6)}</span>
            <span className="chip-row">
              {CACHE_CHOICES.map((mb) => (
                <button key={mb} type="button" className={`chip${s.stemsCacheMb === mb ? " is-on" : ""}`} aria-pressed={s.stemsCacheMb === mb} onClick={() => set({ stemsCacheMb: mb })}>
                  {megabytes(mb * 1e6)}
                </button>
              ))}
              <button type="button" className="chip" disabled={cacheBytes === 0} onClick={() => void import("@/stems/control").then((c) => c.clearStemsCache())}>
                Очистить
              </button>
            </span>
          </div>
        </Card>
      )}

      <p className="muted small studio-note">
        {neural
          ? engaged
            ? "Сейчас играют четыре дорожки песни. Скорость, тон, реверберация и эквалайзер работают на их сумме."
            : "Песня, которая ещё не разделена, играет как обычная. Когда разделение закончится, звук сам переключится на дорожки."
          : "Лёгкий режим: песня делится не на инструменты, а по тому, где в записи стоит звук, поэтому голос с широкой реверберацией может оставить след. Включите нейросеть, и дорожки будут настоящими."}
      </p>
    </div>
  );
}
