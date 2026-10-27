import { useEffect, useMemo, useState } from "react";
import Dialog from "@/ui/components/Dialog";
import { Segmented } from "@/ui/components/Controls";
import { artistLine } from "@/ui/format";
import { getTrackStats, type TrackStatsSummary } from "./listeningStats";
import { closeTrackStats, useStatsDialog } from "./statsDialog";
import { formatListenTime } from "./format";

type ChartRange = "week" | "month" | "year";

function Bars({ values, labels }: { values: number[]; labels: string[] }) {
  const [hover, setHover] = useState<number | null>(null);
  const max = Math.max(1, ...values);
  const width = 560;
  const height = 150;
  const gap = values.length > 14 ? 3 : 8;
  const bar = (width - gap * (values.length - 1)) / values.length;
  return (
    <div className="bars">
      <svg viewBox={`0 0 ${width} ${height + 18}`} role="img" aria-label="Прослушивания по дням">
        {[0.25, 0.5, 0.75, 1].map((f) => (
          <line key={f} x1={0} x2={width} y1={height - height * f} y2={height - height * f} className="bars-grid" />
        ))}
        {values.map((v, i) => {
          const h = v === 0 ? 2 : Math.max(4, (v / max) * (height - 8));
          return (
            <g key={i} onMouseEnter={() => setHover(i)} onMouseLeave={() => setHover(null)}>
              <rect x={i * (bar + gap)} y={0} width={bar} height={height} fill="transparent" />
              <rect x={i * (bar + gap)} y={height - h} width={bar} height={h} rx={Math.min(4, bar / 2)} className={v === 0 ? "bar is-empty" : hover === i ? "bar is-hover" : "bar"} />
            </g>
          );
        })}
        {labels.map((label, i) =>
          label ? (
            <text key={i} x={i * (bar + gap) + bar / 2} y={height + 14} textAnchor="middle" className="bars-label">
              {label}
            </text>
          ) : null,
        )}
      </svg>
      {hover !== null && (
        <div className="bars-tip" style={{ left: `${((hover * (bar + gap) + bar / 2) / width) * 100}%` }}>
          {values[hover]}
        </div>
      )}
    </div>
  );
}

function Stat({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="stat">
      <span className="stat-label">{label}</span>
      <span className="stat-value">{value}</span>
      {sub && <span className="stat-sub">{sub}</span>}
    </div>
  );
}

const longDate = (iso?: string) => (iso ? new Date(iso).toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: "numeric" }) : "—");

export default function TrackStatsDialog() {
  const track = useStatsDialog((s) => s.track);
  const [range, setRange] = useState<ChartRange>("month");
  const [stats, setStats] = useState<TrackStatsSummary | null>(null);

  useEffect(() => {
    if (!track) return;
    const refresh = () => setStats(getTrackStats(track.id));
    refresh();
    window.addEventListener("stats:changed", refresh);
    return () => window.removeEventListener("stats:changed", refresh);
  }, [track]);

  const chart = useMemo(() => {
    if (!stats) return { values: [], labels: [] };
    if (range === "year") {
      return {
        values: stats.monthly.map((m) => m.n),
        labels: stats.monthly.map((m) => new Date(`${m.month}-01T00:00:00`).toLocaleDateString("ru-RU", { month: "short" }).replace(".", "")),
      };
    }
    const days = range === "week" ? stats.daily.slice(-7) : stats.daily;
    return {
      values: days.map((d) => d.n),
      labels: days.map((d, i) => {
        const date = new Date(`${d.day}T00:00:00`);
        if (range === "week") return date.toLocaleDateString("ru-RU", { weekday: "short" });
        return i % 5 === 4 || i === days.length - 1 ? String(date.getDate()) : "";
      }),
    };
  }, [stats, range]);

  return (
    <Dialog title="Статистика трека" open={Boolean(track)} onClose={closeTrackStats} wide>
      {track && stats && (
        <div className="stats-body">
          <div className="stats-track">
            <span className="stats-art">{track.artwork ? <img src={track.artwork} alt="" /> : null}</span>
            <span className="track-text">
              <span className="track-title">{track.title}</span>
              <span className="track-artist">{artistLine(track.artists)}</span>
            </span>
          </div>
          <div className="stats-grid">
            <Stat label="За неделю" value={String(stats.week.n)} sub={formatListenTime(stats.week.ms)} />
            <Stat label="За месяц" value={String(stats.month.n)} sub={formatListenTime(stats.month.ms)} />
            <Stat label="За всё время" value={String(stats.all.n)} sub={formatListenTime(stats.all.ms)} />
          </div>
          <Segmented
            label="Период графика"
            value={range}
            onChange={setRange}
            options={[
              { value: "week", label: "Неделя" },
              { value: "month", label: "30 дней" },
              { value: "year", label: "Год" },
            ]}
          />
          {stats.all.n === 0 ? <p className="muted center">Этот трек ещё не слушали — статистика появится после первых 5 секунд.</p> : <Bars values={chart.values} labels={chart.labels} />}
          <dl className="stats-facts">
            <div><dt>Первое прослушивание</dt><dd>{longDate(stats.first)}</dd></div>
            <div><dt>Последнее</dt><dd>{longDate(stats.last)}</dd></div>
            <div><dt>В среднем в день (месяц)</dt><dd>{(stats.month.n / 30).toFixed(1)}</dd></div>
            <div><dt>Среднее прослушивание</dt><dd>{stats.all.n ? formatListenTime(stats.all.ms / stats.all.n) : "—"}</dd></div>
            {track.plays !== undefined && <div><dt>Прослушиваний на {track.source === "youtube" ? "YouTube" : "SoundCloud"}</dt><dd>{track.plays.toLocaleString("ru-RU")}</dd></div>}
          </dl>
        </div>
      )}
    </Dialog>
  );
}
