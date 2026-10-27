import { useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { DB_QUESTION_STATE_LABELS, type DbQuestionDetailDto } from '../api/dbTypes.js';
import { AgentRuns } from '../components/db/AgentRuns.js';
import { DbQuestionRelations } from '../components/db/DbQuestionRelations.js';
import { DispatchLogDialog } from '../components/db/DispatchLogDialog.js';
import { DbUnavailable } from '../components/db/DbUnavailable.js';
import { DecisionForm } from '../components/DecisionForm.js';
import { ErrorState } from '../components/ErrorState.js';
import { LoadingState } from '../components/LoadingState.js';
import { QuestionCommentForm } from '../components/QuestionCommentForm.js';
import { QuestionConflictDialog } from '../components/QuestionConflictDialog.js';
import { QuestionHistory } from '../components/QuestionHistory.js';
import { ResearchBlock } from '../components/ResearchBlock.js';
import { historyEntries, optionTitle, researchDtos } from '../dbquestion/questionDtos.js';
import { useDbQuestion } from '../dbquestion/useDbQuestion.js';
import { useQuestionResearch } from '../dbquestion/useQuestionResearch.js';
import { MarkdownView } from '../markdown/MarkdownView.js';

const ORIGIN: Record<string, string> = { human: 'Mensch', session: 'Session', subagent: 'Subagent' };

// Entscheidung einer beantworteten oder verworfenen Frage mit Begründung, Person, Datum und „Wieder öffnen“
function DbDecisionBox({ question, busy, onReopen }: { question: DbQuestionDetailDto; busy: boolean; onReopen: () => void }) {
  const d = question.decision;
  if (!d) return null;
  const title = d.option ? optionTitle(question, d.option) : null;
  const answer = d.option ? `Option ${d.option}${title ? ` · ${title}` : ''}` : d.text;
  return (
    <section className={`card decision-box ${d.outcome}`} aria-label="Entscheidung">
      <span className="muted">{d.outcome === 'answered' ? 'Entscheidung' : 'Verworfen'}</span>
      {d.outcome === 'answered' && <strong>{answer}</strong>}
      {d.reason && <span>{d.reason}</span>}
      <span className="muted">
        {d.byName} · {new Date(d.at).toLocaleDateString('de-DE')}
      </span>
      <div>
        <button type="button" className="btn" disabled={busy} onClick={onReopen}>
          Wieder öffnen
        </button>
      </div>
    </section>
  );
}

// Detail einer Frage der Board-Datenbank; je Nummer eine eigene Instanz, damit ein Wechsel (Bezug auf eine andere Frage)
// Auswahl, Eingaben, Konflikt und Fehler zurücksetzt.
export function DbQuestionDetailPage() {
  const { id } = useParams();
  return <DbQuestionDetail key={id} />;
}

// Kopf, Beschreibung, Recherchen, Entscheiden oder Entscheidung, Verlauf, Kommentar, Bezüge.
function DbQuestionDetail() {
  const { pid = '', id } = useParams();
  const q = useDbQuestion(id);
  const research = useQuestionResearch(id, () => void q.refresh());
  const [log, setLog] = useState<string | null>(null);
  const [picked, setPicked] = useState<string | null>(null);
  const [text, setText] = useState('');
  const [reason, setReason] = useState('');
  const [reasonError, setReasonError] = useState<string | null>(null);

  if (q.unavailable) {
    return (
      <div className="page">
        <DbUnavailable pid={pid} message={q.unavailable} onRetry={() => void q.retry()} />
      </div>
    );
  }
  if (q.state.status === 'loading') return <div className="page"><LoadingState variant="detail" text="Frage wird geladen …" /></div>;
  if (q.state.status === 'error') {
    return (
      <div className="page">
        <ErrorState message="Frage konnte nicht geladen werden" detail={q.state.message} onRetry={q.retry} />
      </div>
    );
  }
  const question = q.state.data;
  const open = question.state === 'open';
  // Eine neue Recherche (Live) ohne die gewählte Option hebt die Auswahl auf
  const currentOptions = question.research.at(-1)?.options ?? [];
  if (picked !== null && !currentOptions.some((o) => o.id === picked)) setPicked(null);
  const back = `/p/${pid}/questions?project=${encodeURIComponent(question.projectKey)}`;

  // Entscheidet mit Option oder Freitext; bei Erfolg wird das Formular geleert
  const decide = async () => {
    const ok = await q.decide(picked !== null ? { option: picked, reason: reason.trim() } : { text: text.trim(), reason: reason.trim() });
    if (ok) {
      setPicked(null);
      setText('');
      setReason('');
    }
  };

  // Verwerfen verlangt eine Begründung
  const discard = async () => {
    if (!reason.trim()) {
      setReasonError('Bitte eine Begründung angeben.');
      return;
    }
    setReasonError(null);
    if (await q.discard(reason.trim())) setReason('');
  };

  return (
    <div className="page question-detail">
      <div className="crumb-row">
        <Link className="btn" to={back}>
          Zurück zu den Fragen
        </Link>
        <nav aria-label="Brotkrumen" className="muted">
          <Link to={back}>Fragen</Link> / <span className="mono" aria-current="page">{question.id}</span>
        </nav>
      </div>
      <header className="question-detail-head">
        <div className="question-detail-ident">
          <span className="mono muted">{question.id}</span>
          <span className={`badge q-${question.state}`}>{DB_QUESTION_STATE_LABELS[question.state]}</span>
          {question.ready && <span className="badge q-ready">bereit zur Entscheidung</span>}
          {question.researched && <span className="badge q-researched">recherchiert</span>}
        </div>
        <h1>{question.title}</h1>
        <div className="question-meta">
          {question.tags.map((t) => (
            <span key={t} className="tag">
              {t}
            </span>
          ))}
          <span className="muted">
            Herkunft: {question.originName} ({ORIGIN[question.originKind] ?? question.originKind}{question.originSession ? ` ${question.originSession}` : ''})
            {question.occasion ? ` · Anlass „${question.occasion}“` : ''} · angelegt {new Date(question.createdAt).toLocaleDateString('de-DE')}
          </span>
        </div>
      </header>
      {q.conflict && <QuestionConflictDialog conflict={q.conflict} onChoose={(choice) => void q.resolveConflict(choice)} />}
      <div className="question-detail-grid">
        <div className="question-detail-main">
          <section className="card panel roomy" aria-label="Beschreibung">
            <h2 className="h3">Beschreibung</h2>
            {question.description.trim() ? <MarkdownView source={question.description} /> : <p className="muted">(keine Beschreibung)</p>}
          </section>
          <ResearchBlock
            questionId={question.id}
            research={researchDtos(question.research)}
            selectable={open}
            picked={picked}
            optionsDisabled={text.trim() !== ''}
            onPick={setPicked}
            command={`/ai-harness:board-question ${question.id}`}
            start={{ open, running: research.run, pausedUntil: research.pausedUntil, busy: research.busy, error: research.error, onStart: () => void research.start(), onShowLog: setLog }}
          />
          <section className="card panel roomy" aria-label="Recherche-Läufe">
            <h2 className="h3">Recherche-Läufe</h2>
            <AgentRuns id={question.id} refresh={research.run?.id ?? ''} empty="Zu dieser Frage gab es noch keinen Recherche-Lauf." />
          </section>
          {open ? (
            <DecisionForm
              picked={picked}
              pickedTitle={picked ? optionTitle(question, picked) : null}
              text={text}
              reason={reason}
              busy={q.busy}
              error={reasonError ?? q.error}
              onText={setText}
              onReason={(value) => {
                setReason(value);
                setReasonError(null);
              }}
              onDecide={() => void decide()}
              onDiscard={() => void discard()}
            />
          ) : (
            <DbDecisionBox question={question} busy={q.busy} onReopen={() => void q.reopen()} />
          )}
          {!open && q.error && (
            <p role="alert" className="field-error">
              {q.error}
            </p>
          )}
          <QuestionHistory entries={historyEntries(question)} />
          <QuestionCommentForm busy={q.busy} error={q.commentError} onSend={q.comment} hint="Sessions sehen Kommentare mit „harness board question show“." />
        </div>
        <DbQuestionRelations
          pid={pid}
          question={question}
          busy={q.busy}
          error={q.relationError}
          onSetTickets={q.setTickets}
          onSetDepends={q.setDepends}
        />
      </div>
      {log && <DispatchLogDialog name={log} onClose={() => setLog(null)} />}
    </div>
  );
}
