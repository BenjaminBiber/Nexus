import { Link, useParams } from 'react-router-dom';
import type { DbTicketDetailDto } from '../../api/dbTypes.js';
import { DbDrawer } from './DbDrawer.js';

// Abschnitt „Fragen“ im Ticket-Panel: offene Fragen, auf die das Ticket wartet, und „Frage stellen“.
export function DbTicketQuestions({ ticket, onAsk }: { ticket: DbTicketDetailDto; onAsk: () => void }) {
  const { pid = '' } = useParams();
  const open = ticket.waitingOnQuestions;
  const projectKey = ticket.id.split('-')[0];
  return (
    <DbDrawer
      id="questions" title="Fragen" className="db-ticket-questions" meta={open.length ? `${open.length} offen` : undefined}
      actions={<button type="button" className="btn" onClick={onAsk}>Frage stellen</button>}
    >
      {open.length === 0 ? (
        <p className="muted">Keine offene Frage. Fehlt eine Entscheidung, stell eine Frage – das Ticket wartet dann darauf.</p>
      ) : (
        <>
          <p className="muted">
            Wartet auf {open.length === 1 ? 'eine Frage' : `${open.length} Fragen`}: In Arbeit und Erledigt sind gesperrt, bis{' '}
            {open.length === 1 ? 'sie entschieden ist' : 'alle entschieden sind'}.
          </p>
          <ul className="db-rel-list">
            {open.map((q) => (
              <li key={q.id} className="db-rel-item">
                <Link to={`/p/${pid}/questions/${q.id}`}>
                  <span className="mono">{q.id}</span> {q.title}
                </Link>
              </li>
            ))}
          </ul>
        </>
      )}
      <Link className="muted" to={`/p/${pid}/questions?project=${encodeURIComponent(projectKey)}`}>
        Alle Fragen des Projekts
      </Link>
    </DbDrawer>
  );
}
