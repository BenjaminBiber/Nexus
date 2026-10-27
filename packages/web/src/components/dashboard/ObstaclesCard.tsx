import type { DispatchStatusDto } from '../../api/dispatchTypes.js';
import type { DispatchStatsDto } from '../../api/statsTypes.js';

/** Gründe der Bereitschaft, die auf ein anderes Ticket, einen Merge oder eine Frage warten */
const WAITING = /^wartet auf (?:Merge von |Frage )?/i;
/** Gründe, die an einem Label hängen (Opt-in fehlt oder Opt-out gesetzt) */
const LABEL = /^Label „/;
/** So viele IDs nennt die Zeile „Durch Abhängigkeit gesperrt“ */
const IDS = 3;

interface Obstacle { count: number | null; label: string; sub: string }

// Hindernisse aus Dispatcher-Zustand (Bereitschaft der Tickets) und Auswertung (Pausen, hängende Läufe)
export function obstacles(status: DispatchStatusDto | null, data: DispatchStatsDto): Obstacle[] {
  const tickets = status && !status.ticketsError ? status.tickets : null;
  const waiting = tickets?.filter((t) => WAITING.test(t.reason)) ?? [];
  const blockers = [...new Set(waiting.flatMap((t) => t.reason.match(/[A-Z][A-Z0-9]*-\d+/g) ?? []))];
  const labelled = tickets?.filter((t) => LABEL.test(t.reason)) ?? [];
  const labels = [...new Set(labelled.map((t) => t.reason))];
  return [
    {
      count: tickets ? waiting.length : null, label: 'Durch Abhängigkeit gesperrt',
      sub: blockers.length ? `bereit, warten auf ${blockers.slice(0, IDS).join(', ')}${blockers.length > IDS ? ' …' : ''}` : 'bereit, warten auf andere Tickets',
    },
    { count: tickets ? labelled.length : null, label: 'Per Label ausgenommen', sub: labels.length ? labels.join(' · ') : 'Opt-in fehlt oder Opt-out gesetzt' },
    { count: data.usagePauses, label: 'Nutzungslimit-Pausen', sub: 'im Zeitraum' },
    { count: data.staleRuns, label: 'Hängende Läufe', sub: 'staleMinutes überschritten, im Zeitraum' },
  ];
}

// Karte „Hindernisse“: was den Dispatcher gerade bremst; ohne Dispatcher-Zustand stehen die ersten beiden Zahlen als „–“
export function ObstaclesCard({ status, data }: { status: DispatchStatusDto | null; data: DispatchStatsDto }) {
  return (
    <section className="dash-card obstacles" aria-labelledby="dash-obstacles">
      <h2 id="dash-obstacles">Hindernisse</h2>
      <ul>
        {obstacles(status, data).map((o) => (
          <li key={o.label}>
            <span className="mono obstacle-count">{o.count ?? '–'}</span>
            <span className="obstacle-text"><strong>{o.label}</strong><span className="muted">{o.sub}</span></span>
          </li>
        ))}
      </ul>
    </section>
  );
}
