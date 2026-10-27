import { useState } from 'react';
import type { DispatchStatsDto } from '../../api/statsTypes.js';
import { formatDay, isoDay } from '../../dashboard/formatStats.js';

/** Bis zu so vielen Tagen steht unter jeder Säule ein Datum, darüber unter jeder vierten */
const LABEL_ALL = 14;

// Obergrenze der Achse: Vielfaches von 4, mindestens 4 (Mitte bleibt ganzzahlig)
export function axisMax(max: number): number {
  return Math.ceil(Math.max(4, max) / 4) * 4;
}

// Gestapelte Säulen je Tag oder Woche: erfolgreich, abgebrochen, Frage/blockiert; Zahlen beim Überfahren
export function RunsChart({ series }: { series: DispatchStatsDto['series'] }) {
  const [hover, setHover] = useState(-1);
  const { points, unit } = series;
  const top = axisMax(Math.max(0, ...points.map((p) => p.done + p.aborted + p.other)));
  const pct = (n: number) => `${(n / top) * 100}%`;
  const when = (start: string) => {
    if (unit === 'day') return formatDay(start);
    const [y, m, d] = start.split('-').map(Number);
    return `${formatDay(start)}–${formatDay(isoDay(new Date(y, m - 1, d + 6)))}`;
  };
  const total = points.reduce((s, p) => s + p.done + p.aborted + p.other, 0);
  return (
    <figure className="runs-chart" aria-label={`Läufe je ${unit === 'day' ? 'Tag' : 'Woche'}: ${total} in ${points.length} ${unit === 'day' ? 'Tagen' : 'Wochen'}`}>
      <div className="runs-chart-body">
        <div className="runs-axis mono" aria-hidden="true"><span>{top}</span><span>{top / 2}</span><span>0</span></div>
        <div className="runs-plot-wrap">
          <div className="runs-plot" onMouseLeave={() => setHover(-1)}>
            {points.map((p, i) => (
              <div
                key={p.start} className={`runs-slot${hover === i ? ' hover' : ''}`} data-start={p.start} onMouseEnter={() => setHover(i)}
                title={`${when(p.start)}: ${p.done} erfolgreich, ${p.aborted} abgebrochen, ${p.other} Frage / blockiert`}
              >
                <div className="runs-seg done" style={{ height: pct(p.done) }} />
                <div className="runs-seg fail" style={{ height: pct(p.aborted) }} />
                <div className="runs-seg other" style={{ height: pct(p.other) }} />
                {hover === i && (
                  <div className="runs-tip" role="tooltip">
                    <strong>{when(p.start)}</strong>
                    <span className="mono done">{p.done} erfolgreich</span>
                    <span className="mono fail">{p.aborted} abgebrochen</span>
                    <span className="mono other">{p.other} Frage / blockiert</span>
                  </div>
                )}
              </div>
            ))}
          </div>
          <div className="runs-labels mono" aria-hidden="true">
            {points.map((p, i) => (
              <span key={p.start}>{unit === 'week' || points.length <= LABEL_ALL || i % 4 === 0 ? formatDay(p.start) : ''}</span>
            ))}
          </div>
        </div>
      </div>
    </figure>
  );
}

