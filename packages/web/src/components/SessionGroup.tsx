import { Link } from 'react-router-dom';
import type { SessionDto } from '../api/types.js';
import { Badge } from './Badge.js';
import { SessionRow } from './SessionRow.js';

// Gruppe der Sessions eines Tickets (oder „Nicht zugeordnet“ mit Hinweis auf claim).
export function SessionGroup(props: { pid: string; ticket: { id: string; title: string } | null; sessions: SessionDto[]; now: number; favorites?: Set<string>; showEndedAgents?: boolean; onToggleFavorite?: (session: SessionDto) => void }) {
  const { ticket } = props;
  const running = props.sessions.some((s) => s.displayState === 'running');
  return (
    <section className="card group" aria-label={ticket ? `${ticket.id} ${ticket.title}` : 'Nicht zugeordnet'}>
      <div className="group-head">
        {ticket ? (
          <>
            <span className="mono muted">{ticket.id}</span>
            <h2>{ticket.title}</h2>
            {running && <Badge kind="running">Session läuft</Badge>}
            <Link to={`/p/${props.pid}/board/t/${encodeURIComponent(ticket.id)}`} className="push">
              Ticket öffnen
            </Link>
          </>
        ) : (
          <>
            <h2>Nicht zugeordnet</h2>
            <span className="muted">Sessions ohne Ticket. Mit harness board claim ARC-0042 --agent claude ordnest du sie zu.</span>
          </>
        )}
      </div>
      {props.sessions.map((session) => (
        <SessionRow key={session.id} session={session} href={`/p/${props.pid}/sessions/${encodeURIComponent(session.id)}`} now={props.now} favorite={props.favorites?.has(session.id)} showEndedAgents={props.showEndedAgents} onToggleFavorite={props.onToggleFavorite} />
      ))}
    </section>
  );
}
