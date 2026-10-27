import type { DispatchStatsDto } from '../../api/statsTypes.js';
import { formatPercent, plural } from '../../dashboard/formatStats.js';

/** So viele Gründe zeigt die Karte */
const TOP = 5;
/** Kategorien mit eigener Farbe; der Rest (Infrastruktur, Sonstiges) ist grau */
const COLORED = new Set(['max-turns', 'timeout', 'context', 'exit-code', 'claude-error', 'no-outcome']);

// Karte „Häufigste Abbruchgründe“: die fünf häufigsten Kategorien mit Anteil an allen Abbrüchen und Balken (am häufigsten
// Grund gemessen). Genaue Zahlen und der Anteil, den die Einstellung beeinflusst, stehen im Tooltip.
export function AbortReasonsCard({ data }: { data: DispatchStatsDto }) {
  const reasons = data.abortReasons.slice(0, TOP);
  const total = data.abortReasons.reduce((s, r) => s + r.count, 0);
  const max = Math.max(1, ...reasons.map((r) => r.count));
  const configurable = data.configurableAbortShare !== null
    ? `${formatPercent(data.configurableAbortShare)} der Abbrüche liegen an Limits oder Kontext und lassen sich per Konfig beeinflussen.`
    : undefined;
  return (
    <section className="dash-card" aria-labelledby="dash-reasons">
      <h2 id="dash-reasons" title={configurable}>Häufigste Abbruchgründe</h2>
      {reasons.length === 0 ? (
        <p className="muted dash-empty">Keine Abbrüche im Zeitraum.</p>
      ) : (
        <ul className="bar-list">
          {reasons.map((r) => {
            const cat = `cat-${COLORED.has(r.category) ? r.category : 'rest'}`;
            return (
              <li key={r.category} title={`${r.label}: ${plural(r.count, 'Abbruch', 'Abbrüche')} von ${total} (${formatPercent(r.count / total)})`}>
                <div className="bar-head">
                  <span className="reason-name">{r.label}</span>
                  <span className="bar-runs muted">{plural(r.count, 'Lauf', 'Läufe')}</span>
                  <span className={`bar-value mono reason-text ${cat}`}>{formatPercent(r.count / total)}</span>
                </div>
                <div className="reason-track" aria-hidden="true">
                  <div className={`reason-fill ${cat}`} style={{ width: `${(r.count / max) * 100}%` }} />
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
