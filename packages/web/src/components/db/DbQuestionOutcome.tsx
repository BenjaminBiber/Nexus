import { Link } from 'react-router-dom';
import type { DbQuestionDetailDto } from '../../api/dbTypes.js';
import { optionTitle } from '../../dbquestion/questionDtos.js';
import { dateTimeDe } from '../../dbquestion/questionFormat.js';

// Entscheidung einer beantworteten oder verworfenen Frage: Antwort (Option oder eigene Antwort), Begründung, wer und wann,
// Kennzeichen des Autopiloten und „Wieder öffnen“.
export function DbQuestionOutcome({ question, busy, onReopen }: { question: DbQuestionDetailDto; busy: boolean; onReopen: () => void }) {
  const d = question.decision;
  if (!d) return null;
  const answered = d.outcome === 'answered';
  const title = d.option ? optionTitle(question, d.option) : null;
  return (
    <section className={`qx-outcome ${d.outcome}`} aria-label="Entscheidung">
      <span className="material-icons qx-outcome-icon" aria-hidden="true">
        {answered ? 'check' : 'remove'}
      </span>
      <div className="qx-outcome-body">
        <div className="qx-outcome-head">
          <span className="qx-label">{answered ? 'Entscheidung' : 'Verworfen'}</span>
          {answered && <span className="qx-chip ok">{d.option ? `Option ${d.option}` : 'eigene Antwort'}</span>}
          {d.reason.startsWith('Autopilot:') && <span className="qx-chip">vom Autopilot entschieden</span>}
        </div>
        {answered && <p className="qx-outcome-answer">{d.option ? (title ?? `Option ${d.option}`) : d.text}</p>}
        {d.reason && <p className="qx-outcome-reason">{d.reason}</p>}
        <p className="qx-outcome-by">
          {d.byName} · {dateTimeDe(d.at)}
        </p>
      </div>
      <button type="button" className="qx-btn" disabled={busy} onClick={onReopen}>
        Wieder öffnen
      </button>
    </section>
  );
}

// Hinweis der Recherche auf eine weitere Frage mit „Als neue Frage anlegen“; ist sie schon angelegt, führt ein Verweis zu ihr
export function DbQuestionFollowUp({ title, text, existing, onCreate }: {
  title: string;
  text: string;
  existing: { id: string; href: string } | null;
  onCreate: () => void;
}) {
  return (
    <section className="qx-followup" aria-label="Folgefrage aus der Recherche">
      <div className="qx-followup-head">
        <span className="material-icons" aria-hidden="true">
          help_outline
        </span>
        <strong>{title}</strong>
      </div>
      <p>{text}</p>
      {existing ? (
        <p className="qx-followup-done">
          Angelegt als{' '}
          <Link to={existing.href} className="mono">
            {existing.id}
          </Link>
        </p>
      ) : (
        <button type="button" className="qx-btn info" onClick={onCreate}>
          Als neue Frage anlegen
        </button>
      )}
    </section>
  );
}
