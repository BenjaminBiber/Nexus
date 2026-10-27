import type { DispatchStatsDto, ProfileRowDto } from '../../api/statsTypes.js';
import { formatPercent, plural } from '../../dashboard/formatStats.js';

// Profile in der Reihenfolge der Eskalationsleiter, danach weitere Profile mit Läufen (nach Zahl der Läufe)
export function ladderRows(rows: ProfileRowDto[], ladder: string[]): ProfileRowDto[] {
  const empty = (profile: string): ProfileRowDto => ({ profile, runs: 0, done: 0, escalated: 0, final: 0, other: 0 });
  const onLadder = ladder.map((p) => rows.find((r) => r.profile === p) ?? empty(p));
  return [...onLadder, ...rows.filter((r) => !ladder.includes(r.profile))];
}

// Erläuterung einer Zeile für den Tooltip: Anteile und Zahl der Läufe je Ausgang
export function ladderTitle(r: ProfileRowDto): string {
  if (!r.runs) return 'keine Läufe';
  const part = (n: number, text: string) => `${formatPercent(n / r.runs)} ${text} (${plural(n, 'Lauf', 'Läufe')})`;
  const parts = [part(r.done, 'gelöst')];
  if (r.escalated || !r.final) parts.push(part(r.escalated, 'eskaliert weiter'));
  if (r.final) parts.push(part(r.final, 'endgültig gescheitert'));
  if (r.other) parts.push(part(r.other, 'Frage / blockiert'));
  return parts.join(' · ');
}

// Karte „Eskalation entlang der Leiter“: je Profil Anteil gelöst und Balken (gelöst, eskaliert, endgültig gescheitert);
// darunter der Ausgang der abgeschlossenen Gegenstände. Genaue Zahlen stehen im Tooltip.
export function LadderCard({ data, ladder }: { data: DispatchStatsDto; ladder: string[] }) {
  const rows = ladderRows(data.byProfile, ladder);
  const summary = data.ladder;
  return (
    <section className="dash-card" aria-labelledby="dash-ladder">
      <h2 id="dash-ladder">Eskalation entlang der Leiter</h2>
      {rows.length === 0 ? (
        <p className="muted dash-empty">Keine Läufe im Zeitraum.</p>
      ) : (
        <>
          <ul className="bar-list">
            {rows.map((r) => {
              const segments = [
                { key: 'done', n: r.done, text: 'gelöst' },
                { key: 'escalated', n: r.escalated, text: 'eskaliert weiter' },
                { key: 'final', n: r.final, text: 'endgültig gescheitert' },
              ].filter((s) => s.n > 0);
              return (
                <li key={r.profile} title={ladderTitle(r)}>
                  <div className="bar-head">
                    <span className="mono ladder-name">{r.profile}</span>
                    <span className="bar-runs muted">{plural(r.runs, 'Lauf', 'Läufe')}</span>
                    <span className="bar-value mono tone-good">{r.runs ? formatPercent(r.done / r.runs) : '–'}</span>
                  </div>
                  <div className="ladder-bar" aria-hidden="true">
                    {segments.map((s) => (
                      <div key={s.key} className={s.key} style={{ width: `${(s.n / r.runs) * 100}%` }} title={`${s.text}: ${plural(s.n, 'Lauf', 'Läufe')} (${formatPercent(s.n / r.runs)})`} />
                    ))}
                  </div>
                </li>
              );
            })}
          </ul>
          {summary.finished > 0 && (
            <dl className="ladder-summary" title={`Ausgang von ${summary.finished} abgeschlossenen Tickets bzw. Fragen`}>
              <div><dt className="mono tone-good">{formatPercent(summary.firstTry)}</dt><dd>im 1. Versuch gelöst</dd></div>
              <div><dt className="mono tone-escalated">{formatPercent(summary.afterEscalation)}</dt><dd>nach Eskalation gelöst</dd></div>
              <div><dt className="mono tone-bad">{formatPercent(summary.final)}</dt><dd>endgültig gescheitert</dd></div>
            </dl>
          )}
        </>
      )}
    </section>
  );
}
