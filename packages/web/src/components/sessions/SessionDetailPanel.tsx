import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { Link } from 'react-router-dom';
import type { SessionDto } from '../../api/types.js';
import { useRestoreFocus } from '../../hooks/useRestoreFocus.js';
import { useSessionDetail } from '../../hooks/useSessionDetail.js';
import { formatCost, formatDuration } from '../../sessions/format.js';
import { DispatchLogDialog } from '../db/DispatchLogDialog.js';
import { ErrorState } from '../ErrorState.js';
import { LoadingState } from '../LoadingState.js';
import { SessionStateBadge } from '../SessionStateBadge.js';
import { SessionChangesView } from './SessionChangesView.js';
import { SessionClientBadge } from './SessionClientBadge.js';
import { SessionRunInfo } from './SessionRunInfo.js';
import { SessionTranscript } from './SessionTranscript.js';

type Tab = 'transcript' | 'changes' | 'run';
const TABS: Tab[] = ['transcript', 'changes', 'run'];

const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

// Sichtbare bedienbare Elemente (versteckte Tab-Inhalte zählen nicht)
function focusables(node: HTMLElement): HTMLElement[] {
  return Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => !el.closest('[hidden]'));
}

// Kurzangabe im Kopf mit Symbol
function Fact({ icon, label, children }: { icon: string; label: string; children: ReactNode }) {
  return (
    <span className="sp-fact" title={label}>
      <span className="material-icons" aria-hidden="true">
        {icon}
      </span>
      <span className="sr-only">{label}: </span>
      {children}
    </span>
  );
}

interface Props {
  pid: string;
  sid: string;
  /** Eintrag aus der Liste: Kopf steht sofort, bevor die Details da sind */
  fallback?: SessionDto;
  now: number;
  onClose: () => void;
}

// Slide-Over rechts mit den Details einer Session: Kopf (Titel, Quelle, Zustand, Modell, Laufzeit, Branch, Tickets, Kosten)
// und die Tabs Verlauf, Änderungen und Lauf. Modal (Fokus bleibt im Panel), Esc schließt, der Fokus geht beim Schließen zurück.
export function SessionDetailPanel({ pid, sid, fallback, now, onClose }: Props) {
  const data = useSessionDetail(pid, sid);
  const [tab, setTab] = useState<Tab>('transcript');
  const [log, setLog] = useState<string | null>(null);
  const panelRef = useRef<HTMLElement | null>(null);
  const tabRefs = useRef<Record<Tab, HTMLButtonElement | null>>({ transcript: null, changes: null, run: null });
  const baseId = useId();
  const titleId = `${baseId}-title`;
  useRestoreFocus(panelRef);
  // Beim Öffnen holt das Panel den Fokus, damit Esc und Tab sofort greifen
  useEffect(() => {
    const node = panelRef.current;
    if (node && !node.contains(document.activeElement)) node.focus();
  }, []);

  const detail = data.detail.status === 'ready' ? data.detail.data : null;
  const session = detail?.session ?? fallback ?? null;
  const short = sid.slice(0, 8);
  const title = session?.title || `Session ${short}`;
  const changeCount = data.changes.status === 'ready' ? data.changes.data.files.length : null;

  const labels: Record<Tab, string> = {
    transcript: 'Verlauf',
    changes: changeCount === null ? 'Änderungen' : `Änderungen (${changeCount})`,
    run: 'Lauf',
  };

  // Esc schließt; Tab und Umschalt+Tab bleiben im Panel
  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === 'Escape') {
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== 'Tab' || event.defaultPrevented) return;
    const node = panelRef.current;
    if (!node) return;
    const items = focusables(node);
    if (!items.length) return;
    const first = items[0]!;
    const last = items[items.length - 1]!;
    const active = document.activeElement;
    if (event.shiftKey && (active === first || active === node)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
  };

  // Pfeiltasten, Pos1 und Ende wechseln den Tab (Muster „Tabs“ der WAI-ARIA-Praktiken)
  const onTabKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const index = TABS.indexOf(tab);
    const next =
      event.key === 'ArrowRight' ? TABS[(index + 1) % TABS.length]
      : event.key === 'ArrowLeft' ? TABS[(index + TABS.length - 1) % TABS.length]
      : event.key === 'Home' ? TABS[0]
      : event.key === 'End' ? TABS[TABS.length - 1]
      : null;
    if (!next) return;
    event.preventDefault();
    setTab(next);
    tabRefs.current[next]?.focus();
  };

  const tickets = session ? (session.ticketInfo.length ? session.ticketInfo : session.tickets.map((id) => ({ id, title: '', status: '' }))) : [];
  const tabId = (t: Tab) => `${baseId}-tab-${t}`;
  const panelId = (t: Tab) => `${baseId}-panel-${t}`;

  return (
    <>
      <div className="sheet-backdrop" onClick={onClose} aria-hidden="true" />
      <aside ref={panelRef} tabIndex={-1} className="sheet session-panel" role="dialog" aria-modal="true" aria-labelledby={titleId} onKeyDown={onKeyDown}>
        <header className="sheet-head sp-head">
          <div className="sp-heading">
            <div className="sp-kicker">
              {detail && <SessionClientBadge client={detail.client} />}
              {!detail && session && <SessionClientBadge client={session.client} />}
              {session && <SessionStateBadge state={session.displayState} />}
              <span className="mono muted" title={sid}>
                {short}
              </span>
            </div>
            <h2 id={titleId}>{title}</h2>
            {session && (
              <div className="sp-facts">
                {session.model && <Fact icon="memory" label="Modell"><span className="mono">{session.model}</span></Fact>}
                <Fact icon="schedule" label="Laufzeit">{formatDuration(session.runtimeMs)}</Fact>
                {detail?.branch && <Fact icon="alt_route" label="Branch"><span className="mono">{detail.branch}</span></Fact>}
                {tickets.length > 0 && (
                  <Fact icon="confirmation_number" label="Tickets">
                    {tickets.map((t, i) => (
                      <span key={t.id}>
                        {i > 0 && ', '}
                        <Link to={`/p/${pid}/board/t/${encodeURIComponent(t.id)}`} title={t.title || undefined} className="mono">
                          {t.id}
                        </Link>
                      </span>
                    ))}
                  </Fact>
                )}
                {detail?.costUsd != null && <Fact icon="payments" label="Kosten">{formatCost(detail.costUsd)}</Fact>}
              </div>
            )}
          </div>
          <button type="button" className="icon-btn sp-close" aria-label="Schließen" title="Schließen (Esc)" onClick={onClose}>
            <span className="material-icons" aria-hidden="true">
              close
            </span>
          </button>
        </header>

        <div role="tablist" aria-label="Session-Details" className="tabs sp-tabs" onKeyDown={onTabKey}>
          {TABS.map((t) => (
            <button
              key={t}
              ref={(el) => {
                tabRefs.current[t] = el;
              }}
              type="button"
              role="tab"
              id={tabId(t)}
              aria-selected={tab === t}
              aria-controls={panelId(t)}
              tabIndex={tab === t ? 0 : -1}
              className={`tab${tab === t ? ' on' : ''}`}
              onClick={() => setTab(t)}
            >
              {labels[t]}
            </button>
          ))}
        </div>

        <div role="tabpanel" id={panelId('transcript')} aria-labelledby={tabId('transcript')} hidden={tab !== 'transcript'} className="sp-panel sp-transcript">
          <SessionTranscript transcript={data.transcript} active={tab === 'transcript'} running={session?.displayState === 'running'} onRetry={data.retryTranscript} />
        </div>
        <div role="tabpanel" id={panelId('changes')} aria-labelledby={tabId('changes')} hidden={tab !== 'changes'} className="sp-panel sp-scroll">
          <SessionChangesView pid={pid} sid={sid} changes={data.changes} version={data.changesVersion} onRetry={data.retryChanges} />
        </div>
        <div role="tabpanel" id={panelId('run')} aria-labelledby={tabId('run')} hidden={tab !== 'run'} className="sp-panel sp-scroll">
          {detail ? (
            <SessionRunInfo pid={pid} detail={detail} now={now} onOpenLog={setLog} />
          ) : data.detail.status === 'error' ? (
            <ErrorState message="Details konnten nicht geladen werden" detail={data.detail.message} onRetry={data.retryDetail} />
          ) : (
            <LoadingState text="Laufinfos werden geladen …" />
          )}
        </div>
      </aside>
      {log && <DispatchLogDialog name={log} onClose={() => setLog(null)} />}
    </>
  );
}
