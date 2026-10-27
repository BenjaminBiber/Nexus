import { useId, useState } from 'react';
import { Link } from 'react-router-dom';
import { useApi } from '../../api/ApiContext.js';
import { MERGE_PHASE_LABELS, RUN_KIND_LABELS, type DispatchMergeDto, type DispatchRunDto, type DispatchStatusDto, type DispatchTicketRowDto } from '../../api/dispatchTypes.js';
import { dispatchSteps, isReady, unlocks, type DispatchStep } from '../../dbboard/dispatchSteps.js';
import { formatDate } from '../../dbboard/formatDate.js';
import { DbModal } from './DbModal.js';
import { DispatchLogDialog } from './DispatchLogDialog.js';

// Uhrzeit (HH:MM) eines Zeitpunkts
function clock(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
}

function Icon({ name }: { name: string }) {
  return <span className="material-icons" aria-hidden="true">{name}</span>;
}

// Übersicht des Dispatchers: Zustand und Steuerung, laufende Worker und Merges als Karten, Geplant-Tickets nach Schritten, Fehler.
export function DispatchDialog({ boardId, status, error: loadError, pid, onClose, onStatus, onReload, onOpenTicket, onChanged }: {
  boardId: number | null;
  status: DispatchStatusDto | null;
  error: string | null;
  pid: string;
  onClose: () => void;
  onStatus: (status: DispatchStatusDto) => void;
  onReload: () => void;
  onOpenTicket: (id: string) => void;
  onChanged: () => void;
}) {
  const api = useApi();
  const titleId = useId();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [log, setLog] = useState<string | null>(null);
  const board = boardId ?? undefined;

  // Führt eine Steuerung aus und übernimmt den neuen Zustand
  const act = async (call: () => Promise<DispatchStatusDto>) => {
    setBusy(true);
    setError(null);
    try {
      onStatus(await call());
      onChanged();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const research = status?.research;

  return (
    <>
      <DbModal labelledBy={titleId} onEscape={() => (log ? setLog(null) : onClose())} onBackdrop={onClose} className="dd-modal">
        <header className="dd-head">
          <h2 id={titleId}>Dispatcher</h2>
          {status && <span className={`dd-state state-${status.state}`}>{status.message}</span>}
          {status?.lastTick && <span className="dd-checked">geprüft {formatDate(status.lastTick)}</span>}
          <div className="dd-tools">
            {status && (
              <>
                <button type="button" className="dd-tool primary" disabled={busy} aria-label="Jetzt prüfen" title="Jetzt prüfen" onClick={() => void act(() => api.runDispatch(board))}>
                  <Icon name="refresh" />
                </button>
                {status.state === 'paused'
                  ? <button type="button" className="dd-tool" disabled={busy} aria-label="Fortsetzen" title="Fortsetzen" onClick={() => void act(() => api.resumeDispatch(board))}><Icon name="play_arrow" /></button>
                  : <button type="button" className="dd-tool" disabled={busy} aria-label="Pausieren (60 min)" title="Pausieren (60 min)" onClick={() => void act(() => api.pauseDispatch(60, board))}><Icon name="pause" /></button>}
                <button
                  type="button"
                  className={`dd-tool${status.enabled ? '' : ' off'}`}
                  disabled={busy}
                  aria-label={status.enabled ? 'Ausschalten' : 'Einschalten'}
                  title={status.enabled ? 'Ausschalten' : 'Einschalten'}
                  onClick={() => void act(() => api.setDispatchEnabled(!status.enabled, board))}
                >
                  <Icon name="power_settings_new" />
                </button>
              </>
            )}
            <Link className="dd-tool" to={`/p/${pid}/settings#dispatch`} aria-label="Einstellungen" title="Einstellungen"><Icon name="settings" /></Link>
            <span className="dd-tools-sep" aria-hidden="true" />
            <button type="button" className="dd-tool plain" aria-label="Schließen" title="Schließen" onClick={onClose}><Icon name="close" /></button>
          </div>
        </header>
        {!status && loadError && (
          <div className="dd-pad">
            <div className="banner" role="alert">
              {loadError} <button type="button" className="btn" onClick={onReload}>Erneut laden</button>
            </div>
          </div>
        )}
        {!status && !loadError && <p className="muted dd-pad">Wird geladen …</p>}
        {status && (
          <div className="dd-body">
            <div className="dd-main">
              <div className="dd-stats">
                {research && (
                  <>
                    <span>Recherchen <strong>{research.active}/{research.wip}</strong></span>
                    <span>Automatik <strong>{research.auto ? 'an' : 'aus'}</strong></span>
                    {!!research.maxPerDay && <span>Heute automatisch <strong>{research.today ?? 0}/{research.maxPerDay}</strong></span>}
                  </>
                )}
                <button type="button" className="link-btn dd-log" onClick={() => setLog('dispatch.log')}>dispatch.log anzeigen</button>
              </div>
              {loadError && <div className="banner" role="alert">Stand evtl. veraltet: {loadError}</div>}
              {status.configError && <div className="banner" role="alert">{status.configError}</div>}
              {error && <div className="banner" role="alert">{error}</div>}
              {research?.error && (
                <p className="muted">
                  Recherche-Automatik{research.retryAt ? ` ausgesetzt bis ${formatDate(research.retryAt)}` : ''}: {research.error}
                </p>
              )}
              {!status.boardInScope && boardId !== null && <p className="muted">Dieses Board gehört nicht zu den Boards des Dispatchers (Einstellungen → Dispatcher).</p>}
              {status.runs.length === 0 && status.merges.length === 0 ? (
                <p className="dd-empty">Keine laufenden Worker.</p>
              ) : (
                <ul className="dd-cards" aria-label="Laufende Worker">
                  {status.runs.map((r) => (
                    <RunCard key={r.id} run={r} pid={pid} freed={unlocks(r.ticketId, status.tickets)} onClose={onClose} onOpenTicket={onOpenTicket} onLog={setLog} />
                  ))}
                  {status.merges.map((m) => <MergeCard key={m.ticketId} merge={m} onOpenTicket={onOpenTicket} />)}
                </ul>
              )}
              {status.lastErrors.length > 0 && (
                <section className="dd-errors">
                  <h3>Fehler beim letzten Durchlauf</h3>
                  <ul>{status.lastErrors.map((e, i) => <li key={i}>{e}</li>)}</ul>
                </section>
              )}
            </div>
            <Planned tickets={status.tickets} error={status.ticketsError} onOpenTicket={onOpenTicket} />
          </div>
        )}
      </DbModal>
      {log && <DispatchLogDialog name={log} onClose={() => setLog(null)} />}
    </>
  );
}

function RunCard({ run, pid, freed, onClose, onOpenTicket, onLog }: {
  run: DispatchRunDto;
  pid: string;
  freed: number;
  onClose: () => void;
  onOpenTicket: (id: string) => void;
  onLog: (name: string) => void;
}) {
  return (
    <li className="dd-card">
      <div className="dd-card-top">
        {run.kind === 'research'
          ? <Link className="dd-id" to={`/p/${pid}/questions/${run.questionId ?? run.ticketId}`} onClick={onClose}>{run.questionId ?? run.ticketId}</Link>
          : <button type="button" className="link-btn dd-id" onClick={() => onOpenTicket(run.ticketId)}>{run.ticketId}</button>}
        <span className={`dd-kind kind-${run.kind}`}>{RUN_KIND_LABELS[run.kind] ?? run.kind}</span>
        <span className="dd-time" title={`seit ${formatDate(run.startedAt)}`}><Icon name="schedule" />{clock(run.startedAt)}</span>
      </div>
      <p className="dd-card-title">{run.title}</p>
      <div className="dd-card-foot">
        <span title={`Profil ${run.profile}`}>{run.model ?? 'Standard'}/{run.effort ?? 'Standard'}</span>
        {freed > 0 && <span className="dd-frees">gibt {freed} frei →</span>}
        <button type="button" className="link-btn dd-protocol" aria-label={`Protokoll ${run.log}`} onClick={() => onLog(run.log)}>Protokoll</button>
      </div>
    </li>
  );
}

function MergeCard({ merge, onOpenTicket }: { merge: DispatchMergeDto; onOpenTicket: (id: string) => void }) {
  return (
    <li className="dd-card">
      <div className="dd-card-top">
        <button type="button" className="link-btn dd-id" onClick={() => onOpenTicket(merge.ticketId)}>{merge.ticketId}</button>
        <span className="dd-kind kind-merge">Merge</span>
      </div>
      <p className="dd-card-title">{MERGE_PHASE_LABELS[merge.phase]}</p>
      <div className="dd-card-foot">
        <span className="dd-branch" title={`${merge.branch}${merge.target ? ` → ${merge.target}` : ''}`}>{`${merge.branch}${merge.target ? ` → ${merge.target}` : ''}`}</span>
      </div>
    </li>
  );
}

function Planned({ tickets, error, onOpenTicket }: { tickets: DispatchTicketRowDto[]; error: string | null; onOpenTicket: (id: string) => void }) {
  const steps = dispatchSteps(tickets);
  // Aufgeklappte Schritte; anfangs der erste
  const [open, setOpen] = useState<Set<number>>(() => new Set([1]));
  const toggle = (n: number) => setOpen((prev) => {
    const next = new Set(prev);
    if (!next.delete(n)) next.add(n);
    return next;
  });
  return (
    <aside className="dd-planned" aria-label="Geplant">
      <h3>Geplant</h3>
      {tickets.length > 0 && <p className="dd-planned-sum">{tickets.length} {tickets.length === 1 ? 'Ticket' : 'Tickets'} in {steps.length} {steps.length === 1 ? 'Schritt' : 'Schritten'}</p>}
      {error && <div className="banner" role="alert">{error}</div>}
      {tickets.length === 0 ? <p className="muted">Keine Tickets auf Geplant.</p> : (
        <ol className="dd-steps">
          {steps.map((s) => <StepItem key={s.number} step={s} open={open.has(s.number)} onToggle={() => toggle(s.number)} onOpenTicket={onOpenTicket} />)}
        </ol>
      )}
    </aside>
  );
}

function StepItem({ step, open, onToggle, onOpenTicket }: { step: DispatchStep; open: boolean; onToggle: () => void; onOpenTicket: (id: string) => void }) {
  const bodyId = useId();
  const count = step.tickets.length;
  return (
    <li className={`dd-step${open ? ' open' : ''}`}>
      <button type="button" className="dd-step-head" aria-expanded={open} aria-controls={bodyId} onClick={onToggle}>
        <span className="dd-caret" aria-hidden="true">▸</span>
        <span className="dd-step-name">Schritt {String(step.number).padStart(2, '0')}</span>
        {step.next && <span className="dd-next">als Nächstes</span>}
        {!open && <span className="dd-step-preview">{step.tickets.map((t) => t.title).join(' · ')}</span>}
        <span className="dd-step-count">{count} {count === 1 ? 'Ticket' : 'Tickets'}</span>
      </button>
      {open && (
        <ul className="dd-step-body" id={bodyId}>
          {step.tickets.map((t) => (
            <li key={t.id}>
              <button type="button" className="link-btn dd-id" onClick={() => onOpenTicket(t.id)}>{t.id}</button>
              <span className="dd-step-title">
                {t.title}
                <span className={`dd-reason${isReady(t.reason) ? ' ready' : ''}`}>{t.reason}</span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}
