import { useState, type FormEvent } from 'react';
import { Link } from 'react-router-dom';
import { DB_COLUMN_LABELS, type DbQuestionDetailDto } from '../../api/dbTypes.js';
import type { DbListChange } from '../../dbquestion/useDbQuestion.js';

// Fragenummern erkennen (ARC-F0001); alles andere ist ein Ticket
const QUESTION_ID = /^[A-Za-z][A-Za-z0-9]{1,5}-F\d+$/;

// Bezüge einer Frage: betroffene Tickets (warten auf die Frage) und Abhängigkeiten (Fragen oder Tickets) – hinzufügen und entfernen.
export function DbQuestionRelations({ pid, question, busy, error, onSetTickets, onSetDepends }: {
  pid: string;
  question: DbQuestionDetailDto;
  busy: boolean;
  error: string | null;
  onSetTickets: (change: DbListChange) => Promise<boolean>;
  onSetDepends: (change: DbListChange) => Promise<boolean>;
}) {
  const [ticket, setTicket] = useState('');
  const [depend, setDepend] = useState('');
  const waiting = new Set(question.waitingOn.map((w) => w.id));

  // Fügt einen Eintrag hinzu (Großschreibung, keine Doppelten); bei Erfolg wird das Feld geleert
  const add = async (event: FormEvent, value: string, clear: () => void, onSet: (change: DbListChange) => Promise<boolean>) => {
    event.preventDefault();
    const item = value.trim().toUpperCase();
    if (!item) return;
    if (await onSet((current) => (current.includes(item) ? current : [...current, item]))) clear();
  };

  return (
    <aside className="card panel roomy db-question-relations" aria-label="Bezüge">
      <section aria-label="Betroffene Tickets">
        <h2 className="h3">Betroffene Tickets</h2>
        <p className="muted">Diese Tickets warten, solange die Frage offen ist.</p>
        {question.tickets.length === 0 && <p className="muted">Keine.</p>}
        <ul className="db-rel-list">
          {question.tickets.map((t) => (
            <li key={t.id} className="db-rel-item">
              <Link to={`/p/${pid}/board/t/${t.id}`}>
                <span className="mono">{t.id}</span> {t.title}
              </Link>
              <span className="muted">{DB_COLUMN_LABELS[t.status]}</span>
              <button type="button" className="icon-btn" aria-label={`${t.id} entfernen`} disabled={busy} onClick={() => void onSetTickets((current) => current.filter((x) => x !== t.id))}>
                ×
              </button>
            </li>
          ))}
        </ul>
        <form className="db-rel-add" onSubmit={(e) => void add(e, ticket, () => setTicket(''), onSetTickets)}>
          <input aria-label="Ticket hinzufügen" placeholder="z. B. ARC-0007" value={ticket} onChange={(e) => setTicket(e.target.value)} />
          <button type="submit" className="btn" disabled={busy || !ticket.trim()}>
            Hinzufügen
          </button>
        </form>
      </section>
      <section aria-label="Abhängig von">
        <h2 className="h3">Abhängig von</h2>
        {question.dependsOn.length === 0 && <p className="muted">Keine.</p>}
        <ul className="db-rel-list">
          {question.dependsOn.map((d) => (
            <li key={d} className="db-rel-item">
              <Link to={QUESTION_ID.test(d) ? `/p/${pid}/questions/${d}` : `/p/${pid}/board/t/${d}`}>
                <span className="mono">{d}</span>
              </Link>
              {waiting.has(d) && <span className="badge q-waiting">noch offen</span>}
              <button type="button" className="icon-btn" aria-label={`${d} entfernen`} disabled={busy} onClick={() => void onSetDepends((current) => current.filter((x) => x !== d))}>
                ×
              </button>
            </li>
          ))}
        </ul>
        <form className="db-rel-add" onSubmit={(e) => void add(e, depend, () => setDepend(''), onSetDepends)}>
          <input aria-label="Abhängigkeit hinzufügen" placeholder="Frage oder Ticket, z. B. ARC-F0002" value={depend} onChange={(e) => setDepend(e.target.value)} />
          <button type="submit" className="btn" disabled={busy || !depend.trim()}>
            Hinzufügen
          </button>
        </form>
      </section>
      {error && (
        <p role="alert" className="field-error">
          {error}
        </p>
      )}
    </aside>
  );
}
