import { useEffect, useState } from "react";
import { useSettings } from "@/settings/store";
import { allowed, COMMON_COUNTRIES, lookupCountry, normalizeCountry, useRegion } from "@/services/region";
import { IconRefresh } from "@/ui/components/Icons";
import { Pill, SwitchRow } from "./parts";

// Settings → Services: which countries SoundCloud is not asked from (see services/region.ts).
export default function RegionGuardCard() {
  const guard = useSettings((s) => s.regionGuard);
  const countries = useSettings((s) => s.regionCountries);
  const set = useSettings((s) => s.set);
  const country = useRegion((s) => s.country);
  const [typed, setTyped] = useState("");
  const [checking, setChecking] = useState(false);

  const look = () => {
    setChecking(true);
    void lookupCountry(true).finally(() => setChecking(false));
  };
  useEffect(() => void lookupCountry(), []);

  const toggle = (code: string) => set({ regionCountries: countries.includes(code) ? countries.filter((c) => c !== code) : [...countries, code] });
  const extra = countries.filter((c) => !(COMMON_COUNTRIES as readonly string[]).includes(c));
  const add = () => {
    const code = normalizeCountry(typed);
    if (code && !countries.includes(code)) set({ regionCountries: [...countries, code] });
    setTyped("");
  };
  const ok = allowed(country, { regionGuard: guard, regionCountries: countries });

  return (
    <section className="svc-card">
      <div className="svc-head">
        <span className="svc-head-text">
          <strong>Страна подключения</strong>
          <span className="muted small">SoundCloud закрывает треки по стране адреса, а аккаунт, замеченный с такого адреса, может остаться помеченным.</span>
        </span>
      </div>
      <SwitchRow
        title="Не обращаться к SoundCloud из выбранных стран"
        hint="Пока включён VPN другой страны, всё работает. Без него песни играют с YouTube Music, а аккаунт не помечают."
        checked={guard}
        onChange={(v) => set({ regionGuard: v })}
      />
      {guard && (
        <>
          <div className="chip-row">
            {COMMON_COUNTRIES.map((code) => (
              <button key={code} type="button" className={`chip${countries.includes(code) ? " is-on" : ""}`} aria-pressed={countries.includes(code)} onClick={() => toggle(code)}>{code}</button>
            ))}
            {extra.map((code) => (
              <button key={code} type="button" className="chip is-on" aria-pressed="true" data-tip="Убрать" onClick={() => toggle(code)}>{code}</button>
            ))}
            <input
              className="chip-input"
              value={typed}
              maxLength={2}
              placeholder="+ код"
              aria-label="Код страны из двух букв"
              spellCheck={false}
              onChange={(e) => setTyped(e.target.value.replace(/[^a-zA-Z]/g, "").toUpperCase())}
              onKeyDown={(e) => e.key === "Enter" && add()}
              onBlur={add}
            />
          </div>
          <div className="svc-actions">
            <Pill tone={country ? (ok ? "ok" : "warn") : "off"}>{country ? `Сейчас: ${country}` : "Страна не определена"}</Pill>
            <span className="muted small">{country ? (ok ? "SoundCloud доступен" : "SoundCloud не трогаю: включите VPN") : "Проверка не ответила: ничего не ограничиваю"}</span>
            <button type="button" className="btn ghost" disabled={checking} onClick={look}>
              {checking ? <span className="spinner small-spin" /> : <IconRefresh size={15} />} Проверить
            </button>
          </div>
        </>
      )}
    </section>
  );
}
