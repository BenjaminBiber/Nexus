import { useMemo, useState, type ReactNode } from 'react';
import { Link } from 'react-router-dom';
import type { DbQuestionDetailDto } from '../../api/dbTypes.js';
import { ORIGIN_KIND, dateDe } from '../../dbquestion/questionFormat.js';
import { followUpDraft, questionParts } from '../../dbquestion/questionSections.js';
import { useDbQuestion } from '../../dbquestion/useDbQuestion.js';
import { useQuestionResearch } from '../../dbquestion/useQuestionResearch.js';
import { ErrorState } from '../ErrorState.js';
import { LoadingState } from '../LoadingState.js';
import { QuestionConflictDialog } from '../QuestionConflictDialog.js';
import { DbAskQuestionDialog } from './DbAskQuestionDialog.js';
import { DbQuestionDecide } from './DbQuestionDecide.js';
import { DbQuestionFollowUp, DbQuestionOutcome } from './DbQuestionOutcome.js';
import { DbQuestionRail } from './DbQuestionRail.js';
import { DbQuestionTabs } from './DbQuestionTabs.js';
import { DbUnavailable } from './DbUnavailable.js';
import { DispatchLogDialog } from './DispatchLogDialog.js';

// Hauptspalte ohne geladene Frage (Laden, Fehler, Board-Datenbank nicht verfügbar)
function Placeholder({ children }: { children: ReactNode }) {
  return (
    <div className="qx-scroll">
      <div className="qx-main">{children}</div>
    </div>
  );
}

// Detail einer Frage neben der Liste: Titel, Entscheidungskarte (offen) bzw. Entscheidung (erledigt), Inhalt in Reitern und
// rechts „Bezüge & Details“. Die Seite legt je Nummer eine eigene Instanz an, damit ein Wechsel Auswahl und Eingaben zurücksetzt.
export function DbQuestionDetail({ pid, id, listHref, followUpId, onChanged, onSkip, onCreated, railOpen, onToggleRail }: {
  pid: string;
  id: string;
  /** Zurück zur Liste (schmale Fenster) */
  listHref: string;
  /** Schon angelegte Folgefrage zu dieser Frage */
  followUpId: string | null;
  /** Die Frage hat sich durch eine Aktion hier geändert (Liste nachladen) */
  onChanged: () => void;
  onSkip: (() => void) | null;
  onCreated: (question: DbQuestionDetailDto) => void;
  railOpen: boolean;
  onToggleRail: () => void;
}) {
  const q = useDbQuestion(id);
  const research = useQuestionResearch(id, () => void q.refresh());
  const [log, setLog] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
  const question = q.state.status === 'ready' ? q.state.data : null;
  const parts = useMemo(() => (question ? questionParts(question) : null), [question]);

  if (q.unavailable) {
    return (
      <Placeholder>
        <DbUnavailable pid={pid} message={q.unavailable} onRetry={() => void q.retry()} />
      </Placeholder>
    );
  }
  if (q.state.status === 'error') {
    return (
      <Placeholder>
        <ErrorState message="Frage konnte nicht geladen werden" detail={q.state.message} onRetry={q.retry} />
      </Placeholder>
    );
  }
  if (!question || !parts) {
    return (
      <Placeholder>
        <LoadingState text="Frage wird geladen …" />
      </Placeholder>
    );
  }

  const open = question.state === 'open';
  const latest = question.research.at(-1) ?? null;
  const decision = question.decision;
  const draft = parts.followUp ? followUpDraft(parts.followUp) : null;
  const followUp = draft
    ? {
        title: open
          ? 'Die Recherche sieht eine weitere Frage'
          : decision && latest && latest.at > decision.at
            ? 'Die Recherche kam nach der Entscheidung und schlägt eine Folgefrage vor'
            : 'Die Recherche sah eine weitere Frage',
        text: draft.description || draft.title,
      }
    : null;
  // Meldet erfolgreiche Änderungen an die Liste weiter
  const changed = (ok: boolean) => {
    if (ok) onChanged();
    return ok;
  };
  const followUpCard = followUp && (
    <DbQuestionFollowUp
      title={followUp.title}
      text={followUp.text}
      existing={followUpId ? { id: followUpId, href: `/p/${pid}/questions/${followUpId}` } : null}
      onCreate={() => setAsking(true)}
    />
  );

  return (
    <>
      <div className="qx-scroll">
        <div className="qx-main">
          <Link className="qx-back" to={listHref}>
            <span className="material-icons" aria-hidden="true">
              arrow_back
            </span>
            Alle Fragen
          </Link>
          <header className="qx-title">
            {!open && (
              <p className="qx-meta-line">
                <span className="mono">{question.id}</span> · {question.originName} ({ORIGIN_KIND[question.originKind] ?? question.originKind}) · angelegt{' '}
                {dateDe(question.createdAt)}
              </p>
            )}
            <h2>{question.title}</h2>
          </header>
          {q.conflict && <QuestionConflictDialog conflict={q.conflict} onChoose={(choice) => void q.resolveConflict(choice).then(onChanged)} />}
          {open ? (
            <DbQuestionDecide
              options={latest?.options ?? []}
              decideText={parts.decide?.body ?? null}
              aboutText={parts.about?.body ?? null}
              todayText={parts.today}
              busy={q.busy}
              serverError={q.error}
              onDecide={async (input) => changed(await q.decide(input))}
              onDiscard={async (reason) => changed(await q.discard(reason))}
              onSkip={onSkip}
            />
          ) : (
            <>
              <DbQuestionOutcome question={question} busy={q.busy} onReopen={() => void q.reopen().then(changed)} />
              {q.error && (
                <p role="alert" className="qx-error">
                  {q.error}
                </p>
              )}
              {followUpCard}
            </>
          )}
          <DbQuestionTabs
            question={question}
            parts={parts}
            runRefresh={research.run?.id ?? ''}
            busy={q.busy}
            commentError={q.commentError}
            onComment={q.comment}
          />
        </div>
      </div>
      <DbQuestionRail
        pid={pid}
        question={question}
        busy={q.busy}
        relationError={q.relationError}
        onSetTickets={async (change) => changed(await q.setTickets(change))}
        onSetDepends={async (change) => changed(await q.setDepends(change))}
        research={{ ...research, onStart: () => void research.start(), onShowLog: setLog }}
        followUpCard={open ? followUpCard : null}
        hasFollowUp={open && followUp !== null && !followUpId}
        open={railOpen}
        onToggle={onToggleRail}
      />
      {log && <DispatchLogDialog name={log} onClose={() => setLog(null)} />}
      {asking && draft && (
        <DbAskQuestionDialog
          projectKey={question.projectKey}
          followUp={{ questionId: question.id, title: draft.title, description: draft.description }}
          onClose={() => setAsking(false)}
          onCreated={(created) => {
            setAsking(false);
            onCreated(created);
          }}
        />
      )}
    </>
  );
}
