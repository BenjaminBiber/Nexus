import { useState } from 'react';
import { DB_COLUMN_LABELS, DB_PRIORITY_LABELS, DB_STATUSES, type DbCardDto, type DbStatus } from '../../api/dbTypes.js';
import { MERGE_PHASE_LABELS, type DispatchMergeDto } from '../../api/dispatchTypes.js';
import { initials } from '../ClaudeUsageMeter.js';
import { ClaudeIcon, GitMergeIcon } from '../NavIcons.js';

// Ein Blockiert-Grund „Rückfrage“ wartet auf eine Antwort des Nutzers
const QUESTION_REASON = /^\s*r(ü|ue)ckfrage\s*$/i;

// Karte eines DB-Tickets: ID, Titel und Kennzeichen mit Text (auch offene Fragen); ziehbar (außer in „Wartet“),
// das Statusmenü ersetzt das Ziehen für die Tastatur. Mit agentPrefix zeigt ein Abzeichen „Agent“ Tickets des Dispatchers.
// In der kompakten Ansicht (compact) liegen die Tags eingeklappt hinter einem Umschalter, das Abzeichen „Agent“ wird
// zum bloßen Claude-Logo und Zuständiger, Plan, Handbuch, Diagramm und Kommentarzahl entfallen; statt des
// Fortschrittsbalkens steht nur die Schrittzahl rechts neben dem Tag-Umschalter.
// Mergt der Dispatcher das Ticket gerade (merge: Phase), zeigt die Karte eine Pille „Merge“, kompakt ein animiertes Merge-Symbol.
// Arbeitet gerade ein Agent-Lauf am Ticket (active), pulsiert kompakt das Claude-Logo – auch ohne Agent als Zuständigen;
// links davon steht das Kürzel des Claude-Accounts, mit dem der Lauf arbeitet (account: Name).
export function DbCard({ card, onOpen, onMove, agentPrefix, compact, merge, active, account }: { card: DbCardDto; onOpen: (id: string) => void; onMove: (id: string, status: DbStatus) => void; agentPrefix?: string; compact?: boolean; merge?: DispatchMergeDto['phase']; active?: boolean; account?: string }) {
  const blocked = card.status === 'blocked' ? (QUESTION_REASON.test(card.blockedReason ?? '') ? 'Rückfrage' : 'Blockiert') : null;
  const questions = card.waitingOnQuestions;
  const isAgent = Boolean(agentPrefix && card.assignee?.startsWith(agentPrefix));
  // Der Umschalter des Tag-Drawers darf nicht im Öffnen-Knopf liegen, deshalb steht er unterhalb davon; eingeklappt startet er
  const [tagsOpen, setTagsOpen] = useState(false);
  return (
    <article
      className={`card db-card${compact ? ' compact' : ''}${card.waiting ? ' is-waiting' : ''}`}
      draggable={!card.waiting}
      data-id={card.id}
      onDragStart={(event) => {
        event.dataTransfer.setData('text/plain', card.id);
        event.dataTransfer.effectAllowed = 'move';
      }}
    >
      <button type="button" className="db-card-open" onClick={() => onOpen(card.id)}>
        <span className="card-top">
          <span className="card-id">{card.id}</span>
          <span className="chips end">
            {merge &&
              (compact ? (
                <span className="card-merge-icon" role="img" aria-label="Merge" title={MERGE_PHASE_LABELS[merge]}>
                  <GitMergeIcon />
                </span>
              ) : (
                <span className="badge running" title={MERGE_PHASE_LABELS[merge]}>Merge</span>
              ))}
            {compact && active && account && (
              <span className="card-agent-account" title={`Account: ${account}`}>
                {initials(account)}
              </span>
            )}
            {compact && (isAgent || active) && (
              <span
                className={`card-agent-logo${active ? ' active' : ''}`}
                role="img"
                aria-label={active ? 'Agent arbeitet' : 'Agent'}
                title={active ? 'Agent arbeitet gerade am Ticket' : `Dispatcher: ${card.assignee}`}
              >
                <ClaudeIcon />
              </span>
            )}
            {!compact && isAgent && <span className="badge session" title={`Dispatcher: ${card.assignee}`}>Agent</span>}
            {card.priority !== 'normal' && <span className={`badge prio-${card.priority}`}>{DB_PRIORITY_LABELS[card.priority]}</span>}
            {questions.length > 0 && (
              <span className="badge question" title={questions.map((q) => `${q.id} ${q.title}`).join('\n')}>
                Frage
              </span>
            )}
            {blocked && (
              <span className="badge blocked" title={card.blockedReason ?? ''}>
                {blocked}
              </span>
            )}
          </span>
        </span>
        <span className="card-title">{card.title}</span>
        <span className="chips">
          {!compact && card.assignee && <span className="badge">@{card.assignee}</span>}
          {card.needsPlanning && <span className="badge waiting">Planung</span>}
          {!compact && card.hasPlan && <span className="badge" title="Plan vorhanden">Plan</span>}
          {!compact && card.hasHandbook && <span className="badge" title="Handbuch vorhanden">Handbuch</span>}
          {!compact && card.hasDiagram && <span className="badge" title="Diagramm vorhanden">Diagramm</span>}
          {!compact && card.commentCount > 0 && <span className="badge">{card.commentCount === 1 ? '1 Kommentar' : `${card.commentCount} Kommentare`}</span>}
          {card.sessions.length > 0 && (
            <span className="badge session" title={card.sessions.join('\n')}>
              {card.sessions.length === 1 ? '1 Session' : `${card.sessions.length} Sessions`}
            </span>
          )}
          {!compact &&
            card.labels.map((label) => (
              <span key={label} className="tag">
                {label}
              </span>
            ))}
        </span>
        {!compact && card.stepsTotal > 0 && (
          <span className="card-steps" title="Erledigte Schritte">
            <span
              className="card-steps-bar"
              role="progressbar"
              aria-label="Erledigte Schritte"
              aria-valuemin={0}
              aria-valuemax={card.stepsTotal}
              aria-valuenow={card.stepsDone}
            >
              <span className={`card-steps-fill${card.stepsDone >= card.stepsTotal ? ' complete' : ''}`} style={{ width: `${Math.round((card.stepsDone / card.stepsTotal) * 100)}%` }} />
            </span>
            <span className="card-steps-text">{card.stepsDone}/{card.stepsTotal} Schritte</span>
          </span>
        )}
        {card.waiting && <span className="card-waiting">wartet auf {card.dependsOpen.join(', ')}</span>}
        {questions.length > 0 && <span className="card-waiting">wartet auf Frage {questions.map((q) => q.id).join(', ')}</span>}
      </button>
      {compact && (card.labels.length > 0 || card.stepsTotal > 0) && (
        <div className="card-tags-drawer">
          <div className="card-foot">
            {card.labels.length > 0 && (
              <button type="button" className="card-tags-toggle" aria-expanded={tagsOpen} aria-controls={`tags-${card.id}`} onClick={() => setTagsOpen(!tagsOpen)}>
                Tags ({card.labels.length})
              </button>
            )}
            {card.stepsTotal > 0 && (
              <span className={`card-steps card-steps-count${card.stepsDone >= card.stepsTotal ? ' complete' : ''}`} title="Erledigte Schritte">
                {card.stepsDone}/{card.stepsTotal} Schritte
              </span>
            )}
          </div>
          {tagsOpen && (
            <span id={`tags-${card.id}`} className="chips card-tags">
              {card.labels.map((label) => (
                <span key={label} className="tag">
                  {label}
                </span>
              ))}
            </span>
          )}
        </div>
      )}
      <select className="card-status" aria-label={`Status von ${card.id} ändern`} value={card.status} onChange={(e) => onMove(card.id, e.target.value as DbStatus)}>
        {DB_STATUSES.map((status) => (
          <option key={status} value={status}>
            {DB_COLUMN_LABELS[status]}
          </option>
        ))}
      </select>
    </article>
  );
}
