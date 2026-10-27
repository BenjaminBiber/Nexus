import { Link } from 'react-router-dom';
import { DB_QUESTION_STATE_LABELS, type DbQuestionDto } from '../../api/dbTypes.js';
import { formatAgo } from '../../sessions/format.js';

const ORIGIN: Record<string, string> = { human: 'Mensch', session: 'Session', subagent: 'Subagent' };

// Liste der Fragen eines Board-Projekts in Server-Reihenfolge (bereite zuerst, älteste zuerst); Tickets führen ins Ticket-Panel.
export function DbQuestionList({ pid, questions, freshId }: { pid: string; questions: DbQuestionDto[]; freshId: string | null }) {
  return (
    <section className="card question-list" aria-label="Fragenliste">
      <div className="question-head" aria-hidden="true">
        <span>Nr.</span>
        <span>
          Frage <span className="question-sort">· bereite zuerst, älteste zuerst</span>
        </span>
        <span>Herkunft</span>
        <span>Kennzeichen</span>
        <span>Alter</span>
      </div>
      <ul>
        {questions.map((q) => {
          const fresh = q.id === freshId;
          return (
            <li key={q.id} className={`question-row${fresh ? ' fresh' : ''}`}>
              <span className="mono muted question-id">{q.id}</span>
              <div className="question-main">
                <span className="question-title">
                  <Link to={`/p/${pid}/questions/${q.id}`}>{q.title}</Link>
                  {fresh && <span className="badge q-new">neu</span>}
                </span>
                <span className="question-meta">
                  {q.tags.map((t) => (
                    <span key={t} className="tag">
                      {t}
                    </span>
                  ))}
                  {q.tickets.map((t) => (
                    <Link key={t.id} className="qlink" to={`/p/${pid}/board/t/${t.id}`} title={t.title}>
                      <span className="muted">Ticket</span> <span className="mono">{t.id}</span>
                    </Link>
                  ))}
                </span>
              </div>
              <span className="question-origin">{ORIGIN[q.originKind] ?? q.originKind}</span>
              <span className="question-marks">
                {q.state !== 'open' && <span className={`badge q-${q.state}`}>{DB_QUESTION_STATE_LABELS[q.state]}</span>}
                {q.ready && <span className="badge q-ready">bereit</span>}
                {q.waitingOn.length > 0 && <span className="badge q-waiting">wartet auf {q.waitingOn.map((w) => w.id).join(', ')}</span>}
                {q.researched && <span className="badge q-researched">recherchiert</span>}
                {q.optionCount > 0 && <span className="badge">{q.optionCount === 1 ? '1 Option' : `${q.optionCount} Optionen`}</span>}
              </span>
              <span className="muted question-age">{formatAgo(q.createdAt)}</span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
