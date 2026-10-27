import { useEffect, useMemo, useState } from "react";
import { acceleratorFromEvent, binding, conflictFor, globalMap, prettyAccelerator, SHORTCUTS, type ShortcutDef, type ShortcutScope } from "@/settings/shortcuts";
import { useSettings } from "@/settings/store";
import { recording } from "@/app/shortcuts";
import { Segmented } from "@/ui/components/Controls";
import { IconKeyboard, IconSearch, IconUndo } from "@/ui/components/Icons";
import { toast } from "@/ui/toasts";
import { Card, Pill, SwitchRow, TabHeader } from "./parts";

function Keys({ acc }: { acc: string | null }) {
  const parts = prettyAccelerator(acc);
  if (!parts.length) return <span className="keys-none">не задано</span>;
  return (
    <span className="keys-combo">
      {parts.map((k, i) => (
        <kbd key={i}>{k}</kbd>
      ))}
    </span>
  );
}

function ShortcutRow({ def, failed }: { def: ShortcutDef; failed: boolean }) {
  const overrides = useSettings((s) => s.shortcuts);
  const set = useSettings((s) => s.set);
  const [listening, setListening] = useState(false);
  const acc = binding(overrides, def);
  const changed = def.id in overrides && overrides[def.id] !== def.default;

  useEffect(() => {
    if (!listening) return;
    recording.active = true;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.code === "Escape") return setListening(false);
      if (e.code === "Backspace" || e.code === "Delete") {
        set({ shortcuts: { ...useSettings.getState().shortcuts, [def.id]: null } });
        return setListening(false);
      }
      const next = acceleratorFromEvent(e);
      if (!next) return; // only a modifier so far
      if (def.scope === "global" && !/(Ctrl|Alt)\+/.test(next) && !/^(F\d+|Media)/.test(next)) {
        toast("Для глобальной клавиши нужен Ctrl или Alt — иначе она перехватит обычный ввод", "error");
        return;
      }
      const current = useSettings.getState().shortcuts;
      const clash = conflictFor(current, def.id, next);
      const updated = { ...current, [def.id]: next };
      if (clash) {
        updated[clash.id] = null;
        toast(`«${prettyAccelerator(next).join(" + ")}» было у «${clash.title}» — там теперь пусто`);
      }
      set({ shortcuts: updated });
      setListening(false);
    };
    window.addEventListener("keydown", onKey, true);
    return () => {
      recording.active = false;
      window.removeEventListener("keydown", onKey, true);
    };
  }, [listening, def, set]);

  return (
    <div className={`key-row${listening ? " is-listening" : ""}`}>
      <span className="set-row-text">
        <span className="set-row-title">{def.title}</span>
        <span className="set-row-hint">{def.hint}</span>
      </span>
      {failed && <Pill tone="warn">занято другой программой</Pill>}
      <button type="button" className="key-btn" onClick={() => setListening((v) => !v)} data-tip={listening ? "Esc — отмена, Backspace — убрать" : "Нажмите, чтобы назначить"}>
        {listening ? <span className="keys-wait">Нажмите сочетание…</span> : <Keys acc={acc} />}
      </button>
      <button
        type="button"
        className="icon-btn"
        aria-label="Вернуть как было"
        data-tip="Вернуть как было"
        disabled={!changed}
        style={{ visibility: changed ? "visible" : "hidden" }}
        onClick={() => {
          const next = { ...useSettings.getState().shortcuts };
          delete next[def.id];
          set({ shortcuts: next });
        }}
      >
        <IconUndo size={15} />
      </button>
    </div>
  );
}

export default function KeysTab() {
  const { globalShortcuts, shortcuts, set } = useSettings();
  const [scope, setScope] = useState<ShortcutScope>("app");
  const [query, setQuery] = useState("");
  const [failed, setFailed] = useState<string[]>([]);

  // ask the main process which system-wide keys it could not take
  useEffect(() => {
    if (!window.evplayer || !globalShortcuts) return setFailed([]);
    void window.evplayer.setGlobalShortcuts(globalMap(shortcuts)).then(setFailed);
  }, [shortcuts, globalShortcuts]);

  const list = useMemo(() => {
    const q = query.trim().toLowerCase();
    return SHORTCUTS.filter((d) => d.scope === scope && (!q || `${d.title} ${d.hint}`.toLowerCase().includes(q)));
  }, [scope, query]);

  return (
    <>
      <TabHeader icon={<IconKeyboard size={22} />} hue={265} title="Клавиши" description="Нажмите на сочетание, чтобы поменять его. Esc — отмена, Backspace — убрать клавишу." />
      <Card>
        <div className="keys-toolbar">
          <Segmented
            label="Где работают"
            value={scope}
            onChange={setScope}
            options={[
              { value: "app", label: "В окне EvPlayer" },
              { value: "global", label: "Везде в системе" },
              { value: "overlay", label: "В мини-плеере" },
            ]}
          />
          <label className="keys-search">
            <IconSearch size={15} />
            <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Найти действие" />
          </label>
        </div>
        {scope === "overlay" && <p className="muted small">Работают, когда мини-плеер в фокусе — кликните по нему один раз, например в игре в оконном режиме.</p>}
        {scope === "global" && (
          <SwitchRow
            title="Глобальные клавиши включены"
            hint="Работают, даже когда EvPlayer свёрнут или в трее. Медиаклавиши клавиатуры работают всегда"
            checked={globalShortcuts}
            onChange={(v) => set({ globalShortcuts: v })}
          />
        )}
        <div className="keys-list">
          {list.map((def) => (
            <ShortcutRow key={def.id} def={def} failed={scope === "global" && globalShortcuts && failed.includes(binding(shortcuts, def) ?? "")} />
          ))}
          {list.length === 0 && <p className="muted small">Ничего не нашлось.</p>}
        </div>
        <button
          type="button"
          className="btn ghost"
          onClick={() => {
            const next = { ...shortcuts };
            for (const d of SHORTCUTS) if (d.scope === scope) delete next[d.id];
            set({ shortcuts: next });
            toast("Клавиши сброшены");
          }}
        >
          <IconUndo size={15} /> Сбросить все в этом разделе
        </button>
      </Card>
    </>
  );
}
