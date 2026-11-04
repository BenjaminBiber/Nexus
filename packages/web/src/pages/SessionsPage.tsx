import { useCallback, useEffect, useMemo, useState } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router-dom';
import type { SessionDto } from '../api/types.js';
import { CopyCommand } from '../components/CopyCommand.js';
import { EmptyState } from '../components/EmptyState.js';
import { ErrorState } from '../components/ErrorState.js';
import { LoadingState } from '../components/LoadingState.js';
import { DEFAULT_STATE_FILTER, SessionFilter, type StateFilter } from '../components/SessionFilter.js';
import { SessionGroup } from '../components/SessionGroup.js';
import { FavoritesPanel } from '../components/FavoritesPanel.js';
import { SessionDetailPanel } from '../components/sessions/SessionDetailPanel.js';
import { STATE_LABEL } from '../sessions/format.js';
import { useFavorites } from '../hooks/useFavorites.js';
import { useSessions } from '../hooks/useSessions.js';

interface Group {
  ticket: { id: string; title: string } | null;
  sessions: SessionDto[];
}

// Reihenfolge der Zustände: Aktives zuerst, Beendetes zuletzt
const STATE_ORDER: Record<SessionDto['displayState'], number> = { running: 0, waiting: 1, idle: 2, stale: 3, ended: 4 };

// Sortiert nach Zustand, innerhalb eines Zustands die zuletzt aktive Session zuerst
function bySessionOrder(a: SessionDto, b: SessionDto): number {
  const byState = STATE_ORDER[a.displayState] - STATE_ORDER[b.displayState];
  if (byState !== 0) return byState;
  return new Date(b.lastEventAt).getTime() - new Date(a.lastEventAt).getTime();
}

// Gruppiert Sessions nach Ticket (aufsteigend); jede Session erscheint genau einmal, und zwar beim Ticket
// mit der kleinsten Nummer. Sessions ohne Ticket kommen in eine eigene letzte Gruppe.
function group(sessions: SessionDto[]): Group[] {
  const byTicket = new Map<string, Group>();
  const loose: SessionDto[] = [];
  for (const session of [...sessions].sort(bySessionOrder)) {
    const [info] = [...session.ticketInfo].sort((a, b) => a.id.localeCompare(b.id));
    if (!info) {
      loose.push(session);
      continue;
    }
    const entry = byTicket.get(info.id) ?? { ticket: { id: info.id, title: info.title }, sessions: [] };
    entry.sessions.push(session);
    byTicket.set(info.id, entry);
  }
  const groups = [...byTicket.values()].sort((a, b) => a.ticket!.id.localeCompare(b.ticket!.id));
  return loose.length ? [...groups, { ticket: null, sessions: loose }] : groups;
}

// Zusammenfassung der Zustände: „1 läuft · 1 wartet auf Eingabe · 1 veraltet“
function summarize(sessions: SessionDto[]): string {
  return (['running', 'waiting', 'stale'] as const)
    .map((state) => [state, sessions.filter((s) => s.displayState === state).length] as const)
    .filter(([, count]) => count > 0)
    .map(([state, count]) => `${count} ${STATE_LABEL[state]}`)
    .join(' · ');
}

// Sessions des Projekts: nach Ticket gruppiert, mit Zuständen, Subagenten und Filter; aktualisiert sich live.
// Unter sessions/:sid liegt das Slide-Over mit den Details einer Session über der Liste (teilbar, Zurück schließt).
export function SessionsPage() {
  const { pid = '', '*': rest = '' } = useParams();
  const sid = rest.split('/')[0] ?? '';
  const navigate = useNavigate();
  const location = useLocation();
  const fromList = (location.state as { fromList?: boolean } | null)?.fromList === true;
  // Aus der Liste geöffnet: zurück im Verlauf (dann öffnet „Vorwärts“ es wieder); per Link geöffnet: zur Liste
  const closeDetail = useCallback(() => {
    if (fromList) navigate(-1);
    else navigate(`/p/${pid}/sessions`, { replace: true });
  }, [fromList, navigate, pid]);
  const { state, retry } = useSessions(pid);
  const favorites = useFavorites(pid);
  const [problem, setProblem] = useState<string | null>(null);
  const [filter, setFilter] = useState<StateFilter>(DEFAULT_STATE_FILTER);
  const [onlyFavorites, setOnlyFavorites] = useState(false);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [now, setNow] = useState(() => Date.now());

  // Laufzeiten und „vor … Min.“ regelmäßig neu berechnen
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 30000);
    return () => clearInterval(timer);
  }, []);

  const all = state.status === 'ready' ? state.data : [];
  const favoriteList = favorites.state.status === 'ready' ? favorites.state.data : [];
  const favoriteIds = useMemo(() => new Set(favoriteList.map((f) => f.sessionId)), [favoriteList]);
  const visible = useMemo(
    () => all.filter((s) => (filter.size === 0 || filter.has(s.displayState)) && (!onlyFavorites || favoriteIds.has(s.id))),
    [all, filter, onlyFavorites, favoriteIds],
  );
  const groups = useMemo(() => group(visible), [visible]);
  const summary = summarize(all);

  // Stern: markieren oder Markierung entfernen; Fehler werden angezeigt
  const toggle = async (session: SessionDto) => {
    setProblem(null);
    try {
      if (favoriteIds.has(session.id)) await favorites.unmark(session.id);
      else await favorites.mark(session.id);
    } catch (error) {
      setProblem((error as Error).message);
    }
  };
  // Notiz oder Entfernen im Favoritenpanel
  const guarded = (action: () => Promise<void>) => async () => {
    setProblem(null);
    try {
      await action();
    } catch (error) {
      setProblem((error as Error).message);
    }
  };

  const detail = sid ? <SessionDetailPanel key={sid} pid={pid} sid={sid} fallback={all.find((s) => s.id === sid)} now={now} onClose={closeDetail} /> : null;

  if (state.status === 'loading') return <div className="page"><LoadingState text="Sessions werden geladen …" />{detail}</div>;
  if (state.status === 'error') return <><ErrorState message={state.message} onRetry={retry} />{detail}</>;

  return (
    <div className="page stack">
      <div className="page-head">
        <h1>Sessions</h1>
        {summary && <span className="muted">{summary}</span>}
        <button type="button" className="fchip fav-toggle push" aria-expanded={sheetOpen} aria-controls="favpanel" onClick={() => setSheetOpen((open) => !open)}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="#e8a317" stroke="#b7791f" strokeWidth="1.5" strokeLinejoin="round" aria-hidden="true">
            <path d="M12 3l2.7 5.6 6.1.8-4.5 4.2 1.1 6.1L12 16.8 6.6 19.7l1.1-6.1L3.2 9.4l6.1-.8z" />
          </svg>
          Favoriten
          <span className="mono muted">{favoriteList.length}</span>
        </button>
      </div>
      <SessionFilter value={filter} onChange={setFilter} onlyFavorites={onlyFavorites} onOnlyFavorites={setOnlyFavorites} />
      {problem && (
        <p role="alert" className="field-error">
          {problem}
        </p>
      )}
      {sheetOpen && (
        <FavoritesPanel
          pid={pid}
          favorites={favoriteList}
          now={now}
          onNote={(id, note) => guarded(() => favorites.mark(id, note))()}
          onRemove={(id) => guarded(() => favorites.unmark(id))()}
          onClose={() => setSheetOpen(false)}
        />
      )}
      {all.length === 0 ? <Empty /> : groups.length === 0 ? <p className="muted">Keine Sessions mit diesem Zustand.</p> : null}
      {groups.map((g) => (
        <SessionGroup key={g.ticket?.id ?? 'loose'} pid={pid} ticket={g.ticket} sessions={g.sessions} now={now} favorites={favoriteIds} showEndedAgents={filter.has('ended')} onToggleFavorite={(s) => void toggle(s)} />
      ))}
      {detail}
    </div>
  );
}

// Leerzustand mit Hinweisen auf Plugin und Einrichtung
function Empty() {
  return (
    <>
      <div className="panel">
        <EmptyState title="Keine Sessions">Starte ein Ticket, dann erscheint die Session hier.</EmptyState>
      </div>
      <div className="panel note">
        <h2>Noch keine Hook-Meldungen empfangen</h2>
        <p>
          Das Plugin ai-harness meldet Sessions automatisch, sobald das Projekt eingerichtet ist. Führe im Projekt aus:
        </p>
        <CopyCommand command="harness init" />
      </div>
    </>
  );
}
