import { useId, useState, type FormEvent, type ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { DB_COLUMN_LABELS, type DbQuestionDetailDto } from '../../api/dbTypes.js';
import type { DispatchRunDto } from '../../api/dispatchTypes.js';
import { ORIGIN_KIND, QUESTION_ID, dateTimeDe, timeDe } from '../../dbquestion/questionFormat.js';
import type { DbListChange } from '../../dbquestion/useDbQuestion.js';
import { CopyCommand } from '../CopyCommand.js';

// „Recherche starten“: Claude-Session über den Dispatcher (claude -p im eigenen, losgelösten Worktree)
export interface RailResearch {
  run: DispatchRunDto | null;
  /** Pause des Dispatchers (z. B. Nutzungslimit): der Server lehnt den Start bis dahin ab */
  pausedUntil: string | null;
  busy: boolean;
  error: string | null;
  onStart: () => void;
  onShowLog: (name: string) => void;
}

// Eingabe mit Plus-Knopf zum Ergänzen einer Liste (Großschreibung, keine Doppelten); bei Erfolg wird das Feld geleert
function AddField({ label, button, placeholder, busy, onAdd }: {
  label: string;
  button: string;
  placeholder: string;
  busy: boolean;
  onAdd: (change: DbListChange) => Promise<boolean>;
}) {
  const [value, setValue] = useState('');
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const item = value.trim().toUpperCase();
    if (!item) return;
    if (await onAdd((current) => (current.includes(item) ? current : [...current, item]))) setValue('');
  };
  return (
    <form className="qx-add" onSubmit={(event) => void submit(event)}>
      <input aria-label={label} placeholder={placeholder} value={value} onChange={(event) => setValue(event.target.value)} />
      <button type="submit" className="qx-btn icon small" aria-label={button} title={button} disabled={busy || !value.trim()}>
        <span className="material-icons" aria-hidden="true">
          add
        </span>
      </button>
    </form>
  );
}

// Knopf zum Entfernen eines Eintrags
function RemoveButton({ id, busy, onClick }: { id: string; busy: boolean; onClick: () => void }) {
  return (
    <button type="button" className="qx-remove" aria-label={`${id} entfernen`} title={`${id} entfernen`} disabled={busy} onClick={onClick}>
      <span className="material-icons" aria-hidden="true">
        close
      </span>
    </button>
  );
}

// Rechte Leiste „Bezüge & Details“: wartende bzw. betroffene Tickets, Abhängigkeiten, Recherche starten, Herkunft und Daten,
// bei offenen Fragen die Folgefrage aus der Recherche. Eingeklappt bleibt nur ein schmaler Streifen mit Hinweisen.
export function DbQuestionRail({ pid, question, busy, relationError, onSetTickets, onSetDepends, research, followUpCard, hasFollowUp, open, onToggle }: {
  pid: string;
  question: DbQuestionDetailDto;
  busy: boolean;
  relationError: string | null;
  onSetTickets: (change: DbListChange) => Promise<boolean>;
  onSetDepends: (change: DbListChange) => Promise<boolean>;
  research: RailResearch;
  /** Folgefrage aus der Recherche (nur offene Fragen; entschiedene zeigen sie in der Hauptspalte) */
  followUpCard: ReactNode;
  /** Eine noch nicht angelegte Folgefrage (Hinweis am eingeklappten Streifen) */
  hasFollowUp: boolean;
  open: boolean;
  onToggle: () => void;
}) {
  const baseId = useId();
  const isOpen = question.state === 'open';
  const waiting = new Set(question.waitingOn.map((w) => w.id));
  const waitingTickets = isOpen ? question.tickets.length : 0;
  const hints = [
    waitingTickets === 1 ? '1 Ticket wartet' : waitingTickets > 1 ? `${waitingTickets} Tickets warten` : null,
    hasFollowUp ? '1 Folgefrage' : null,
  ].filter(Boolean);
  const pauseHint = isOpen && !research.run && research.pausedUntil
    ? `Der Dispatcher pausiert bis ${timeDe(research.pausedUntil)} (z. B. Nutzungslimit) – eine Recherche startet erst danach.`
    : null;
  const d = question.decision;

  return (
    <aside className={`qx-rail${open ? '' : ' closed'}`} aria-label="Bezüge und Details">
      <div className="qx-rail-body">
        <section className="qx-rail-sec" aria-labelledby={`${baseId}-tickets`}>
          <h3 id={`${baseId}-tickets`}>{isOpen ? 'Wartende Tickets' : 'Betroffene Tickets'}</h3>
          {question.tickets.length === 0 && <p className="qx-muted">{isOpen ? 'Keine – kein Ticket wartet auf diese Frage.' : 'Keine.'}</p>}
          {!isOpen && question.tickets.length > 0 && <p className="qx-muted">Freigegeben – sie warten nicht mehr.</p>}
          {question.tickets.length > 0 && (
            <ul className="qx-rel">
              {question.tickets.map((t) => (
                <li key={t.id}>
                  <span className="qx-rel-main">
                    <Link to={`/p/${pid}/board/t/${t.id}`}>
                      <span className="mono">{t.id}</span> {t.title}
                    </Link>
                    <span className={`qx-rel-state ${isOpen ? 'waiting' : t.status}`}>{isOpen ? 'wartet' : DB_COLUMN_LABELS[t.status]}</span>
                  </span>
                  <RemoveButton id={t.id} busy={busy} onClick={() => void onSetTickets((current) => current.filter((x) => x !== t.id))} />
                </li>
              ))}
            </ul>
          )}
          <AddField label="Ticket hinzufügen" button="Ticket eintragen" placeholder={`z. B. ${question.projectKey}-0007`} busy={busy} onAdd={onSetTickets} />
        </section>

        <section className="qx-rail-sec" aria-labelledby={`${baseId}-depends`}>
          <h3 id={`${baseId}-depends`}>Abhängig von</h3>
          {question.dependsOn.length === 0 && <p className="qx-muted">{isOpen ? 'Keine – die Frage kann entschieden werden.' : 'Keine.'}</p>}
          {question.dependsOn.length > 0 && (
            <ul className="qx-rel">
              {question.dependsOn.map((dep) => (
                <li key={dep}>
                  <span className="qx-rel-main">
                    <Link to={QUESTION_ID.test(dep) ? `/p/${pid}/questions/${dep}` : `/p/${pid}/board/t/${dep}`}>
                      <span className="mono">{dep}</span>
                    </Link>
                    {waiting.has(dep) && <span className="qx-rel-state waiting">noch offen</span>}
                  </span>
                  <RemoveButton id={dep} busy={busy} onClick={() => void onSetDepends((current) => current.filter((x) => x !== dep))} />
                </li>
              ))}
            </ul>
          )}
          <AddField label="Abhängigkeit hinzufügen" button="Abhängigkeit eintragen" placeholder="Frage oder Ticket" busy={busy} onAdd={onSetDepends} />
          {relationError && (
            <p role="alert" className="qx-error">
              {relationError}
            </p>
          )}
        </section>

        <section className="qx-rail-sec" aria-labelledby={`${baseId}-research`}>
          <h3 id={`${baseId}-research`}>Recherche</h3>
          {isOpen ? (
            <button type="button" className="qx-btn soft" disabled={research.run !== null || research.busy} onClick={research.onStart}>
              {question.research.length > 0 ? 'Neue Recherche starten' : 'Recherche starten'}
            </button>
          ) : (
            <p className="qx-muted">Nur offene Fragen lassen sich recherchieren. „Wieder öffnen“ macht es möglich.</p>
          )}
          {pauseHint && <p className="qx-muted">{pauseHint}</p>}
          {research.run && (
            <p role="status" className="qx-muted">
              Recherche läuft – das Ergebnis erscheint hier automatisch (Protokoll{' '}
              <button type="button" className="link-btn" onClick={() => research.onShowLog(research.run!.log)}>
                {research.run.log}
              </button>
              ).
            </p>
          )}
          {research.error && (
            <p role="alert" className="qx-error">
              {research.error}
            </p>
          )}
          <CopyCommand command={`/nexus:board-question ${question.id}`} compact />
        </section>

        <dl className="qx-rail-sec qx-facts">
          <dt>Herkunft</dt>
          <dd>
            {question.originName} · {ORIGIN_KIND[question.originKind] ?? question.originKind}
            {question.originSession ? ` ${question.originSession}` : ''}
          </dd>
          {question.occasion && (
            <>
              <dt>Anlass</dt>
              <dd>{question.occasion}</dd>
            </>
          )}
          {question.tags.length > 0 && (
            <>
              <dt>Tags</dt>
              <dd className="qx-tags">
                {question.tags.map((t) => (
                  <span key={t} className="qx-chip">
                    {t}
                  </span>
                ))}
              </dd>
            </>
          )}
          <dt>Angelegt</dt>
          <dd>{dateTimeDe(question.createdAt)}</dd>
          {d && (
            <>
              <dt>{d.outcome === 'answered' ? 'Entschieden' : 'Verworfen'}</dt>
              <dd>{dateTimeDe(d.at)}</dd>
            </>
          )}
        </dl>

        {followUpCard}
      </div>
      {open ? (
        <button type="button" className="qx-collapse" aria-expanded="true" onClick={onToggle}>
          Bezüge &amp; Details einklappen
          <span className="material-icons" aria-hidden="true">
            chevron_right
          </span>
        </button>
      ) : (
        <button
          type="button"
          className="qx-collapsed"
          aria-expanded="false"
          aria-label={`Bezüge und Details ausklappen${hints.length ? ` – ${hints.join(', ')}` : ''}`}
          title="Bezüge & Details ausklappen"
          onClick={onToggle}
        >
          <span className="material-icons" aria-hidden="true">
            chevron_left
          </span>
          <span className="qx-vertical">Bezüge &amp; Details</span>
          {waitingTickets > 0 && (
            <span className="qx-wait" aria-hidden="true">
              <span className="material-icons">schedule</span>
              {waitingTickets}
            </span>
          )}
          {hasFollowUp && (
            <span className="qx-followup-dot" aria-hidden="true">
              <span className="material-icons">help_outline</span>
            </span>
          )}
        </button>
      )}
    </aside>
  );
}
