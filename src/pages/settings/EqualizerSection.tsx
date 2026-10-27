import { useState } from "react";
import { BAND_COLORS, formatFrequency, MAX_GAIN, MAX_PREAMP, MIN_PREAMP, MAX_Q, MIN_Q, PRESET_IDS, type BandType, type PresetId } from "@/equalizer/eqModel";
import { useEqualizerStore } from "@/equalizer/eqStore";
import EqualizerGraph from "@/equalizer/EqualizerGraph";
import { decodePresetCode, encodePresetCode } from "@/equalizer/presetCode";
import { Row, SwitchRow } from "./parts";
import { IconMore } from "@/ui/components/Icons";
import { openMenu } from "@/ui/menu";
import { toast } from "@/ui/toasts";
import Dialog from "@/ui/components/Dialog";
import { useReorder } from "@/ui/useReorder";

const PRESET_LABEL: Record<PresetId, string> = { flat: "Ровно", rich: "Насыщенно", bass: "Бас", vocal: "Вокал", warm: "Тепло", bright: "Ярко" };
const TYPE_LABEL: Record<BandType, string> = { lowshelf: "Низы (полка)", peaking: "Пик", highshelf: "Верха (полка)" };

/**
 * A number box that lets you type freely: while typing, a value outside the range (the "2"
 * on the way to "27") is just text; it is applied once it fits, and corrected into the
 * range when you leave the box or press Enter.
 */
function NumberField({ label, value, suffix, step, min, max, onChange }: { label: string; value: number; suffix?: string; step: number; min: number; max: number; onChange: (v: number) => void }) {
  const [draft, setDraft] = useState<string | null>(null);
  const shown = String(Number(value.toFixed(step < 1 ? 2 : 0)));
  const commit = () => {
    if (draft === null) return;
    const next = parseFloat(draft.replace(",", "."));
    if (Number.isFinite(next)) onChange(Math.min(max, Math.max(min, next)));
    setDraft(null);
  };
  return (
    <label className="num-field">
      <span>{label}</span>
      <span className="num-field-box">
        <input
          type="text"
          inputMode="decimal"
          value={draft ?? shown}
          onFocus={(e) => e.currentTarget.select()}
          onChange={(e) => {
            const text = e.target.value.replace(/[^\d.,-]/g, "");
            setDraft(text);
            const next = parseFloat(text.replace(",", "."));
            if (Number.isFinite(next) && next >= min && next <= max) onChange(next);
          }}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
            if (e.key === "Escape") {
              setDraft(null);
              e.currentTarget.blur();
            }
            // arrows step the value like a spin box
            if (e.key === "ArrowUp" || e.key === "ArrowDown") {
              e.preventDefault();
              const base = draft !== null && Number.isFinite(parseFloat(draft)) ? parseFloat(draft) : value;
              const next = Math.min(max, Math.max(min, base + (e.key === "ArrowUp" ? step : -step) * (e.shiftKey ? 10 : 1)));
              setDraft(null);
              onChange(next);
            }
          }}
        />
        {suffix && <span className="muted small">{suffix}</span>}
      </span>
    </label>
  );
}

function NameDialog({ title, initial, open, onClose, onSubmit, multiline }: { title: string; initial: string; open: boolean; onClose: () => void; onSubmit: (v: string) => string | void; multiline?: boolean }) {
  const [value, setValue] = useState(initial);
  const [error, setError] = useState("");
  return (
    <Dialog title={title} open={open} onClose={onClose}>
      <form
        className="dialog-form"
        onSubmit={(e) => {
          e.preventDefault();
          const problem = onSubmit(value);
          if (problem) setError(problem);
          else onClose();
        }}
      >
        <label className="field">
          {multiline ? <textarea autoFocus rows={3} value={value} onChange={(e) => { setValue(e.target.value); setError(""); }} spellCheck={false} placeholder="EVQ1.…" /> : <input autoFocus value={value} maxLength={32} onChange={(e) => setValue(e.target.value)} />}
        </label>
        {error && <p className="field-error">{error}</p>}
        <div className="dialog-actions">
          <button type="button" className="btn ghost" onClick={onClose}>Отмена</button>
          <button type="submit" className="btn primary" disabled={!value.trim()}>Готово</button>
        </div>
      </form>
    </Dialog>
  );
}

function CustomPresets() {
  const { customPresets, preset, savePreset, applyCustomPreset, overwritePreset, renamePreset, deletePreset, importPreset, movePreset } = useEqualizerStore();
  // drag a preset left or right to change the order
  const reorder = useReorder(movePreset, "x");
  const [dialog, setDialog] = useState<{ kind: "save" | "rename" | "import"; id?: string; initial: string } | null>(null);
  return (
    <div className="custom-presets">
      <span className="eyebrow">Свои пресеты</span>
      <div className="chip-row">
        {customPresets.map((p, i) => (
          <span key={p.id} className={`chip is-draggable${preset === p.id ? " is-on" : ""}${reorder.hint(i) ? ` drop-${reorder.hint(i)}` : ""}`} {...reorder.rowProps(i)}>
            <button type="button" onClick={() => applyCustomPreset(p.id)}>{p.name}</button>
            <button
              type="button"
              className="chip-more"
              aria-label={`Ещё: ${p.name}`}
              onClick={(e) => {
                const r = e.currentTarget.getBoundingClientRect();
                openMenu({ clientX: r.left, clientY: r.bottom + 4 }, [
                  { label: "Сохранить текущие настройки сюда", onSelect: () => { overwritePreset(p.id); toast(`«${p.name}» обновлён`); } },
                  { label: "Переименовать", onSelect: () => setDialog({ kind: "rename", id: p.id, initial: p.name }) },
                  { label: "Скопировать код пресета", onSelect: () => void navigator.clipboard.writeText(encodePresetCode(p)).then(() => toast("Код скопирован — отправьте его другу")) },
                  { label: "Удалить", danger: true, onSelect: () => deletePreset(p.id) },
                ]);
              }}
            >
              <IconMore size={14} />
            </button>
          </span>
        ))}
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
              const created = savePreset(value);
              toast(`Пресет «${created.name}» сохранён`);
            } else if (dialog.kind === "rename" && dialog.id) {
              renamePreset(dialog.id, value);
            } else {
              try {
                const created = importPreset(decodePresetCode(value.trim()));
                toast(`Пресет «${created.name}» добавлен`);
              } catch {
                return "Код повреждён — скопируйте его целиком";
              }
            }
          }}
        />
      )}
    </div>
  );
}

export default function EqualizerSection() {
  const { enabled, bands, preset, preamp, setEnabled, updateBand, applyPreset, reset, setPreamp, normalize, setNormalize, protect, setProtect, mode, setMode } = useEqualizerStore();
  const [selectedId, setSelectedId] = useState<string | null>(bands[0]?.id ?? null);
  const found = bands.findIndex((b) => b.id === selectedId);
  // after switching modes the old band is gone: show the first one
  const selectedIndex = found >= 0 ? found : bands.length ? 0 : -1;
  const selected = selectedIndex >= 0 ? bands[selectedIndex] : null;

  return (
    <div className="settings-group">
      <SwitchRow title="Эквалайзер" hint={`${bands.length} полос: перетаскивайте точки на графике вверх-вниз (громкость) и влево-вправо (частота)`} checked={enabled} onChange={setEnabled} />
      <Row title="Режим" hint={mode === "simple" ? "Шесть полос, каждую можно поставить на любую частоту" : "Десять полос на классических частотах — от 31 Гц до 16 кГц"}>
        <div className="chip-row">
          <button type="button" className={`chip${mode === "simple" ? " is-on" : ""}`} onClick={() => setMode("simple")}>Обычный</button>
          <button type="button" className={`chip${mode === "advanced" ? " is-on" : ""}`} onClick={() => setMode("advanced")}>Продвинутый</button>
        </div>
      </Row>
      <SwitchRow
        title="Защита от хрипа"
        hint="Приглушает общий уровень на величину усиления и срезает пики. Выключена: бас-буст и любое усиление звучат в полную силу, на громких треках возможен перегруз"
        checked={protect}
        onChange={setProtect}
      />
      <SwitchRow title="Выравнивать громкость" hint="Тихие и громкие треки звучат примерно одинаково — без резких скачков между песнями" checked={normalize} onChange={setNormalize} />
      <div className="chip-row">
        {PRESET_IDS.map((id) => (
          <button key={id} type="button" className={`chip${preset === id ? " is-on" : ""}`} onClick={() => applyPreset(id)}>
            {PRESET_LABEL[id]}
          </button>
        ))}
        {preset === "custom" && <span className="chip is-on">Свой</span>}
        <button type="button" className="reset-btn push-right" onClick={reset}>Сбросить</button>
      </div>
      <CustomPresets />
      <EqualizerGraph
        bands={bands}
        enabled={enabled}
        selectedId={selectedId}
        onSelect={setSelectedId}
        onChange={(id, patch) => {
          if (!enabled) setEnabled(true);
          updateBand(id, patch);
        }}
      />
      <p className="muted small">Колесо мыши над точкой — ширина полосы, двойной клик — обнулить, Shift — двигать только по высоте.</p>
      {selected && (
        <div className="band-card">
          <div className="band-head">
            <span className="band-dot" style={{ background: BAND_COLORS[selectedIndex % BAND_COLORS.length] }} />
            <span className="toggle-title">Полоса {selectedIndex + 1} · {formatFrequency(selected.frequency)}</span>
            <label className="band-on">
              <span className="muted small">Включена</span>
              <input type="checkbox" className="switch" checked={selected.enabled} onChange={(e) => updateBand(selected.id, { enabled: e.target.checked })} />
            </label>
          </div>
          <div className="chip-row">
            {(["lowshelf", "peaking", "highshelf"] as BandType[]).map((type) => (
              <button key={type} type="button" className={`chip${selected.type === type ? " is-on" : ""}`} onClick={() => updateBand(selected.id, { type })}>
                {TYPE_LABEL[type]}
              </button>
            ))}
          </div>
          {/* keyed by band: text typed for one band never lands on the next one */}
          <div className="band-fields" key={selected.id}>
            <NumberField label="Частота" value={selected.frequency} suffix="Гц" step={1} min={20} max={20000} onChange={(v) => updateBand(selected.id, { frequency: v })} />
            <NumberField label="Усиление" value={selected.gain} suffix="дБ" step={0.5} min={-MAX_GAIN} max={MAX_GAIN} onChange={(v) => updateBand(selected.id, { gain: v })} />
            <NumberField label="Ширина (Q)" value={selected.q} step={0.1} min={MIN_Q} max={MAX_Q} onChange={(v) => updateBand(selected.id, { q: v })} />
          </div>
        </div>
      )}
      <Row title="Предусиление" hint="Общий уровень после эквалайзера. Если звук хрипит — опустите его или включите защиту от хрипа">
        <input
          type="range"
          className="dial small-dial"
          min={MIN_PREAMP}
          max={MAX_PREAMP}
          step={0.5}
          value={preamp}
          onChange={(e) => setPreamp(parseFloat(e.target.value))}
          style={{ "--fill": `${((preamp - MIN_PREAMP) / (MAX_PREAMP - MIN_PREAMP)) * 100}%` } as React.CSSProperties}
        />
        <span className="muted small tabular">{preamp > 0 ? "+" : ""}{preamp.toFixed(1)} дБ</span>
      </Row>
    </div>
  );
}
