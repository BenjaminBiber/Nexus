import { Link, useParams } from 'react-router-dom';
import type { DbTicketDto } from '../../api/dbTypes.js';

// Herkunft und Umsetzung eines DB-Tickets: Spec, Zielbranch, Branch, Worktree, Bereiche, Schnittstellen und Sessions (nur lesend; Seitenleiste des Ticket-Panels).
export function DbTicketWork({ ticket }: { ticket: DbTicketDto }) {
  const { pid } = useParams();
  const rows: Array<{ label: string; value: string; mono?: boolean }> = [];
  if (ticket.spec) rows.push({ label: 'Spec', value: ticket.specTask ? `${ticket.spec} / ${ticket.specTask}` : ticket.spec });
  if (ticket.targetBranch) rows.push({ label: 'Zielbranch', value: ticket.targetBranch, mono: true });
  if (ticket.branch) rows.push({ label: 'Branch', value: ticket.branch, mono: true });
  if (ticket.worktree) rows.push({ label: 'Worktree', value: ticket.worktree, mono: true });
  if (ticket.touches.length) rows.push({ label: 'Bereiche', value: ticket.touches.join(', '), mono: true });
  if (ticket.consumes.length) rows.push({ label: 'Nutzt', value: ticket.consumes.join('; ') });
  if (ticket.produces.length) rows.push({ label: 'Liefert', value: ticket.produces.join('; ') });
  if (!rows.length && !ticket.sessions.length) return null;
  return (
    <section className="db-work db-side-section" aria-label="Umsetzung">
      <h3 className="db-side-title">Umsetzung</h3>
      <dl className="db-work-list">
        {rows.map((row) => (
          <div key={row.label}>
            <dt>{row.label}</dt>
            <dd className={row.mono ? 'mono' : undefined}>{row.value}</dd>
          </div>
        ))}
        {ticket.sessions.length > 0 && (
          <div>
            <dt>Sessions</dt>
            <dd>
              {ticket.sessions.map((s) => (
                <span key={s} className="mono db-session" title={s}>
                  {s.slice(0, 8)}
                </span>
              ))}
              {pid && <Link to={`/p/${pid}/sessions`}>Sessions ansehen</Link>}
            </dd>
          </div>
        )}
      </dl>
    </section>
  );
}
