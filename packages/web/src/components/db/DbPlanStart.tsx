import type { DbStatus, DbTicketDetailDto } from '../../api/dbTypes.js';
import { useTicketPlanning } from '../../dbboard/useTicketPlanning.js';
import { CopyCommand } from '../CopyCommand.js';

/** Status, in denen „Planen“ angeboten wird */
const PLANNABLE: ReadonlySet<DbStatus> = new Set<DbStatus>(['backlog', 'in_planning', 'planned', 'blocked']);

// Uhrzeit HH:MM (Ortszeit) einer ISO-Zeit
function hhmm(iso: string): string {
  const d = new Date(iso);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

// Knopf „Planen“ im Ticket: lässt das Ticket sofort von einer Claude-Session (claude -p, nur lesend) planen. Nur auf eigenen
// Boards (`mine`) und in Backlog, In Planung, Geplant oder Blockiert; gesperrt mit Hinweis bei offener Frage, laufender
// Planung oder Pause. Während des Laufs steht dort der Name des Protokolls; danach lädt das Panel das Ticket neu.
export function DbPlanStart({ ticket, mine, onDone }: { ticket: DbTicketDetailDto; mine: boolean; onDone: () => void }) {
  const planning = useTicketPlanning(mine ? ticket.id : undefined, ticket.boardId, onDone);
  if (!mine || !PLANNABLE.has(ticket.status)) return null;
  const question = ticket.waitingOnQuestions.length > 0;
  const hint = planning.run
    ? null
    : question
      ? `Wartet auf Frage ${ticket.waitingOnQuestions.map((q) => q.id).join(', ')} – erst entscheiden, dann planen.`
      : planning.pausedUntil
        ? `Der Dispatcher pausiert bis ${hhmm(planning.pausedUntil)} (z. B. Nutzungslimit) – eine Planung startet erst danach.`
        : null;
  const disabled = question || planning.run !== null || planning.busy || planning.pausedUntil !== null;
  return (
    <section className="db-plan-start db-side-section" aria-label="Planung">
      <h3 className="db-side-title">Planung</h3>
      <div className="actions">
        <button type="button" className={ticket.needsPlanning ? 'btn primary' : 'btn'} disabled={disabled} onClick={() => void planning.start()}>
          Planen
        </button>
        {hint && <span className="muted">{hint}</span>}
      </div>
      {planning.run && (
        <p role="status" className="muted">
          Planung läuft ({planning.run.profile}) – der Plan erscheint hier automatisch (Protokoll <span className="mono">{planning.run.log}</span>).
        </p>
      )}
      {planning.error && (
        <div className="banner" role="alert">
          {planning.error}
        </div>
      )}
      {!planning.run && <CopyCommand command={`nexus board plan-start ${ticket.id}`} compact />}
    </section>
  );
}
