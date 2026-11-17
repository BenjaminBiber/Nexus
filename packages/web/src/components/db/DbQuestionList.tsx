import { useId } from 'react';
import { Link } from 'react-router-dom';
import type { DbQuestionDto } from '../../api/dbTypes.js';
import { dayMonthDe, optionsLabel } from '../../dbquestion/questionFormat.js';
import { formatAgo } from '../../sessions/format.js';

interface Group {
  key: string;
  label: string;
  items: DbQuestionDto[];
}

// Gruppen der Liste: offene nach „Bereit“ (nichts offen, worauf sie wartet) und „Wartet“, erledigte nach „Beantwortet“ und „Verworfen“
export function questionGroups(questions: DbQuestionDto[]): Group[] {
  const groups: Group[] = [
    { key: 'ready', label: 'Bereit', items: questions.filter((q) => q.state === 'open' && q.waitingOn.length === 0) },
    { key: 'waiting', label: 'Wartet', items: questions.filter((q) => q.state === 'open' && q.waitingOn.length > 0) },
    { key: 'answered', label: 'Beantwortet', items: questions.filter((q) => q.state === 'answered') },
    { key: 'discarded', label: 'Verworfen', items: questions.filter((q) => q.state === 'discarded') },
  ];
  return groups.filter((g) => g.items.length > 0);
}

// Zweite Zeile einer Frage: Nummer, Alter bzw. Entscheidungsdatum, Optionen, worauf sie wartet
function metaOf(q: DbQuestionDto): string[] {
  const parts: string[] = [];
  if (q.state === 'open') {
    parts.push(formatAgo(q.createdAt));
    if (q.optionCount > 0) parts.push(optionsLabel(q.optionCount));
    if (q.waitingOn.length > 0) parts.push(`wartet auf ${q.waitingOn.map((w) => w.id).join(', ')}`);
  } else if (q.decision) {
    parts.push(`${q.state === 'answered' ? 'entschieden' : 'verworfen'} ${dayMonthDe(q.decision.at)}`);
  }
  return parts;
}

// Fragenliste der Seite „Fragen“: gruppiert, jede Zeile führt zur Frage (rechts daneben im Detail). Offene Fragen zeigen,
// wie viele Tickets auf sie warten; erledigte, ob sie beantwortet oder verworfen sind.
export function DbQuestionList({ pid, questions, currentId, freshId }: {
  pid: string;
  questions: DbQuestionDto[];
  currentId: string | null;
  freshId: string | null;
}) {
  const baseId = useId();
  return (
    <div className="qx-groups">
      {questionGroups(questions).map((group) => {
        const headingId = `${baseId}-${group.key}`;
        return (
          <section key={group.key} aria-labelledby={headingId}>
            <h2 id={headingId} className="qx-group">
              {group.label} · {group.items.length}
            </h2>
            <ul className="qx-rows">
              {group.items.map((q) => {
                const current = q.id === currentId;
                const waiting = q.state === 'open' ? q.ticketIds.length : 0;
                const waitLabel = waiting === 1 ? '1 Ticket wartet' : `${waiting} Tickets warten`;
                return (
                  <li key={q.id} className={`qx-row${current ? ' current' : ''}${q.id === freshId ? ' fresh' : ''}`}>
                    {q.state === 'answered' && (
                      <span className="material-icons qx-row-icon ok" aria-hidden="true">
                        check
                      </span>
                    )}
                    {q.state === 'discarded' && (
                      <span className="material-icons qx-row-icon" aria-hidden="true">
                        remove
                      </span>
                    )}
                    <span className="qx-row-main">
                      <span className="qx-row-title">
                        <Link className="qx-row-link" to={`/p/${pid}/questions/${q.id}`} aria-current={current ? 'page' : undefined}>
                          {q.title}
                        </Link>
                        {q.id === freshId && <span className="qx-chip new">neu</span>}
                      </span>
                      <span className="qx-row-meta">
                        <span className="mono">{q.id}</span>
                        {metaOf(q).map((part) => ` · ${part}`)}
                      </span>
                    </span>
                    {waiting > 0 && (
                      <span className="qx-wait" title={waitLabel}>
                        <span className="material-icons" aria-hidden="true">
                          schedule
                        </span>
                        <span aria-hidden="true">{waiting}</span>
                        <span className="sr-only">{waitLabel}</span>
                      </span>
                    )}
                  </li>
                );
              })}
            </ul>
          </section>
        );
      })}
    </div>
  );
}
