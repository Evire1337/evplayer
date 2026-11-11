import { IconJam } from "@/ui/components/Icons";
import { useJam } from "./store";

/** 1 слушатель, 2 слушателя, 5 слушателей */
export function listeners(n: number): string {
  const tens = n % 100;
  const ones = n % 10;
  const word = tens >= 11 && tens <= 14 ? "слушателей" : ones === 1 ? "слушатель" : ones >= 2 && ones <= 4 ? "слушателя" : "слушателей";
  return `${n} ${word}`;
}

/** on "Сейчас играет" while a session is on: who is in it; a click opens the session */
export default function JamBanner() {
  const live = useJam((s) => s.status === "live");
  const people = useJam((s) => s.people);
  const host = useJam((s) => s.host);
  const code = useJam((s) => s.code);
  if (!live) return null;
  const hostName = people.find((p) => p.host)?.name ?? "";
  return (
    <button type="button" className="now-jam" onClick={() => useJam.getState().setOpen(true)}>
      <span className="now-jam-who">
        <span className="now-jam-faces">
          {people.slice(0, 4).map((p) => (
            <span key={p.id} className="jam-avatar" style={{ background: p.color }}>{p.name.slice(0, 1).toUpperCase()}</span>
          ))}
        </span>
        <span className="now-jam-text">
          Вместе: {listeners(people.length)}
          <small>{host ? "Вы организатор" : `Организатор: ${hostName}`}</small>
        </span>
      </span>
      <span className="now-jam-code">{code} <IconJam size={16} /></span>
    </button>
  );
}
