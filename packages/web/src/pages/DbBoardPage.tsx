import { useEffect, useMemo, useRef, useState, type CSSProperties, type MouseEvent } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import { ApiError } from '../api/ApiError.js';
import type { DbStatus } from '../api/dbTypes.js';
import { DbAddFab } from '../components/db/DbAddFab.js';
import { DbAcceptanceDialog } from '../components/db/DbAcceptanceDialog.js';
import { DbColumn, WAITING_HINT } from '../components/db/DbColumn.js';
import { DbDoneSections } from '../components/db/DbDoneSections.js';
import { DbGraphDialog, type GraphScopeOption } from '../components/db/DbGraphDialog.js';
import { DbSpecsDialog } from '../components/db/DbSpecsDialog.js';
import { DbImportDialog, importSources } from '../components/db/DbImportDialog.js';
import { DbNewTicketDialog } from '../components/db/DbNewTicketDialog.js';
import { DbNotesDialog } from '../components/db/DbNotesDialog.js';
import { DbTicketPanel, type LeaveGuard } from '../components/db/DbTicketPanel.js';
import { DispatchDialog } from '../components/db/DispatchDialog.js';
import { DispatchIndicator } from '../components/db/DispatchIndicator.js';
import { DbUnavailable } from '../components/db/DbUnavailable.js';
import { ErrorState } from '../components/ErrorState.js';
import { LoadingState } from '../components/LoadingState.js';
import { SearchIcon } from '../components/NavIcons.js';
import { buildDoneLayout } from '../dbboard/doneSections.js';
import { filterDbCards } from '../dbboard/filterDbCards.js';
import { moveDbCard } from '../dbboard/moveDbCard.js';
import { useBoardProjectSelection } from '../dbboard/boardProject.js';
import { useCompactCards } from '../dbboard/compactCards.js';
import { useDbBoard } from '../dbboard/useDbBoard.js';
import { useDbBoardsChanged } from '../dbquestion/activeBoard.js';
import { useDispatchStatus } from '../dbboard/useDispatchStatus.js';
import { useAsync } from '../hooks/useAsync.js';

// Klicks hierauf schließen das Panel nicht: Panel, Dialoge samt Hintergrund, Karten und Bedienelemente behalten ihr Verhalten
const BOARD_CLICK_IGNORED = '.db-panel, [role="dialog"], [role="alertdialog"], .db-modal-backdrop, .diagram-viewer-backdrop, .db-card, button, select, input, textarea, a, label';

export type DbDialog = 'new' | 'import' | 'notes' | 'specs' | 'dispatch' | 'graph' | null;

// DB-Board (Projekt und Board wählt die Seitenleiste): acht Spalten mit Karten, Verschieben per Ziehen oder Statusmenü,
// Ticket-Panel unter board/t/:id und Dialoge (Neues Ticket, Von anderem Board holen, Notizen); die Verwaltung öffnet der Stift neben der Board-Auswahl.
export function DbBoardPage() {
  const { pid = '', '*': rest = '' } = useParams();
  const ticketId = /^t\/([^/]+)$/.exec(rest)?.[1] ?? null;
  const navigate = useNavigate();
  const api = useApi();
  const board = useDbBoard();
  // Änderungen aus der Verwaltung (Seitenleiste) laden Board, Projekte und Ansicht neu
  useDbBoardsChanged(board.reloadAll);
  // Suche im aktuellen Board (ID, Titel, Zuständig, Label); die Suche über alle Boards öffnet der Knopf daneben
  const [query, setQuery] = useState('');
  const [message, setMessage] = useState<{ kind: 'error' | 'info'; text: string } | null>(null);
  const [dialog, setDialog] = useState<DbDialog>(null);
  const [askAcceptance, setAskAcceptance] = useState<string | null>(null);
  // Umfänge des Abhängigkeitsgraphen (Board und ggf. eine Spec) und der zuerst gezeigte
  const [graph, setGraph] = useState<{ scopes: GraphScopeOption[]; initial: number } | null>(null);
  // Ein Projekt ohne Board kommt aus der Seitenleiste einer anderen Seite über den Navigationszustand
  const location = useLocation();
  const [projectKey, setProjectKey] = useState<string | null>(() => (location.state as { project?: string } | null)?.project ?? null);
  const mounted = useRef(true);
  const messageTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const panelGuard = useRef<LeaveGuard | null>(null);
  // Notizen-Dialog eines bestimmten Boards (aus der Suche über den Navigationszustand { notesBoard }), sonst des angezeigten
  const [notesBoard, setNotesBoard] = useState<number | null>(null);
  useEffect(() => {
    const wanted = (location.state as { notesBoard?: unknown } | null)?.notesBoard;
    if (typeof wanted !== 'number') return;
    setNotesBoard(wanted);
    setDialog('notes');
    // Zustand verbrauchen, damit Zurück/Neu laden den Dialog nicht erneut öffnet
    navigate(location.pathname, { replace: true, state: null });
  }, [location.state, location.pathname, navigate]);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      if (messageTimer.current) clearTimeout(messageTimer.current);
    };
  }, []);
  // Info-Hinweise verschwinden nach 4 s (eine neue Meldung startet die Zeit neu); Fehler bleiben
  useEffect(() => {
    if (messageTimer.current) clearTimeout(messageTimer.current);
    messageTimer.current = null;
    if (message?.kind !== 'info') return;
    messageTimer.current = setTimeout(() => setMessage(null), 4000);
  }, [message]);

  const active = board.active.state.status === 'ready' ? board.active.state.data : null;
  const projects = board.projects.state.status === 'ready' ? board.projects.state.data : [];
  const boards = board.boards.state.status === 'ready' ? board.boards.state.data : [];
  const view = board.view.state.status === 'ready' ? board.view.state.data : null;
  const openNotes = board.notes.state.status === 'ready' ? board.notes.state.data.length : 0;
  const shownProject = projectKey ?? active?.board?.projectKey ?? projects[0]?.key ?? '';
  const showsActive = projectKey === null || projectKey === active?.board?.projectKey;
  const dispatch = useDispatchStatus(showsActive ? active?.boardId ?? null : null);
  const compactCards = useCompactCards();
  // Laufende Merges des Dispatchers je Ticket (Pille bzw. Symbol auf der Karte)
  const merges = useMemo(() => new Map((dispatch.status?.merges ?? []).map((m) => [m.ticketId, m.phase])), [dispatch.status]);
  // Namen der Claude-Accounts für das Kürzel auf der kompakten Karte; ohne Liste bleibt die Account-ID stehen
  const accounts = useAsync(() => api.getAccounts(), [api]);
  const accountNames = useMemo(() => new Map(accounts.state.status === 'ready' ? accounts.state.data.accounts.map((a) => [a.id, a.name]) : []), [accounts.state]);
  // Tickets mit laufendem Agent-Lauf (Recherchen gehören zu Fragen, nicht zu Tickets) samt Account-Name – kompakt pulsiert das Claude-Logo
  const activeRuns = useMemo(
    () => new Map((dispatch.status?.runs ?? []).filter((r) => r.kind !== 'research').map((r) => [r.ticketId, r.account ? accountNames.get(r.account) ?? r.account : undefined])),
    [dispatch.status, accountNames],
  );
  const cards = useMemo(() => view?.columns.flatMap((c) => c.tickets) ?? [], [view]);
  // Spalten „Nicht mehr nötig“ und „In Planung“ lassen sich unter Einstellungen › Darstellung ausblenden
  const userSettings = useAsync(() => api.getUserSettings(), [api]);
  const settings = userSettings.state.status === 'ready' ? userSettings.state.data : null;
  const hidden = (id: string) => (id === 'not_needed' && settings?.hideNotNeeded) || (id === 'in_planning' && settings?.hideInPlanning);
  const shownColumns = view?.columns.filter((c) => !hidden(c.id)) ?? [];
  // Die Spalte „Erledigt“ zeigt Zeilen in Abschnitten; bei aktiver Suche sind alle Abschnitte offen und archivierte Treffer sichtbar
  const searching = query.trim() !== '';
  const now = new Date();
  // Boards desselben Projekts, von denen sich Tickets holen lassen
  const sources = active?.board ? importSources(active.board, boards) : [];
  // Gehört das Board dem angemeldeten Benutzer? (für „Mergen“ im Panel)
  const isOwnBoard = (boardId: number) => Boolean(active) && boards.some((b) => b.id === boardId && b.ownerId === active?.user.id);

  // Öffnet den Abhängigkeitsgraphen des Boards; mit spec zuerst den der Spec (Umschalter zwischen beiden)
  const openGraph = (spec?: string) => {
    if (!view) return;
    const scopes: GraphScopeOption[] = [{ label: 'Board', scope: { kind: 'board', boardId: view.boardId } }];
    const project = active?.board?.projectKey;
    if (spec && project) scopes.push({ label: `Spec ${spec}`, scope: { kind: 'spec', project, spec } });
    setGraph({ scopes, initial: scopes.length - 1 });
    setDialog('graph');
  };

  // Wechsel bei offenem Panel laufen über dessen Wache (Rückfrage bei ungespeicherten Eingaben)
  const guarded = (action: () => void) => (panelGuard.current ? panelGuard.current(action) : action());

  // Öffnet ein Ticket im Seitenpanel (eigene Adresse)
  const openTicket = (id: string) => {
    if (id !== ticketId) guarded(() => navigate(`/p/${pid}/board/t/${id}`));
  };

  // Schließt das Panel (zurück zur Board-Adresse)
  const closeTicket = () => navigate(`/p/${pid}/board`);

  // Klick aufs Board (außerhalb von Panel, Dialogen, Karten und Bedienelementen) schließt das Panel über dieselbe Wache wie „Schließen“.
  // Nach dem Ziehen einer Karte zählt der nächste Klick nicht.
  const dragged = useRef(false);
  const onBoardClick = (event: MouseEvent<HTMLElement>) => {
    if (dragged.current) {
      dragged.current = false;
      return;
    }
    if (!ticketId || !(event.target instanceof Element) || event.target.closest(BOARD_CLICK_IGNORED)) return;
    guarded(closeTicket);
  };

  // Wechselt das Board; setzt activeBoard in board.yaml und schließt ein offenes Ticket-Panel
  const switchBoard = async (id: number) => {
    setMessage(null);
    try {
      await board.activate(id);
      setProjectKey(null);
      if (ticketId) closeTicket();
    } catch (error) {
      setMessage({ kind: 'error', text: (error as Error).message });
    }
  };

  // Wechselt das Projekt: das erste Board des Projekts wird aktiv; ohne Board nur die Auswahl zeigen (Panel schließt in beiden Fällen)
  const switchProject = async (key: string) => {
    const first = boards.find((b) => b.projectKey === key);
    if (first) await switchBoard(first.id);
    else {
      setProjectKey(key);
      if (ticketId) closeTicket();
    }
  };

  // Board- und Projektwechsel aus der Board-Auswahl der Seitenleiste (mit Rückfrage bei ungespeicherten Eingaben im Panel)
  useBoardProjectSelection(
    shownProject || null,
    (choice) => guarded(() => void (choice.board !== null ? switchBoard(choice.board) : switchProject(choice.project))),
    showsActive ? active?.boardId ?? null : null,
  );

  // Verschiebt sofort sichtbar und speichert; bei Fehler kehrt die Karte zurück und die Meldung erscheint.
  // Nach „Erledigt“ ohne Abnahme fragt erst der Abnahme-Dialog nach (auch wenn der Server sie vermisst).
  const move = async (id: string, status: DbStatus, acceptance?: string) => {
    const card = cards.find((c) => c.id === id);
    if (!view || !card || card.status === status) return;
    setMessage(null);
    // Offene Fragen sperren „In Arbeit“ und „Erledigt“ (wie der Server); vor dem Abnahme-Dialog und ohne sichtbares Verschieben
    if ((status === 'in_progress' || status === 'done') && card.waitingOnQuestions.length > 0) {
      setMessage({ kind: 'error', text: `Wartet auf Frage ${card.waitingOnQuestions.map((q) => q.id).join(', ')} – erst entscheiden.` });
      return;
    }
    if (status === 'done' && acceptance === undefined && !card.hasAcceptance) {
      setAskAcceptance(id);
      return;
    }
    board.view.set(moveDbCard(view, id, status));
    try {
      await api.moveDbTicket(id, acceptance === undefined ? { status, rev: card.rev } : { status, rev: card.rev, acceptance });
      if (mounted.current) board.view.reload();
    } catch (error) {
      if (!mounted.current) return;
      board.view.set(view);
      board.view.reload(); // frische Revision (409) und zwischenzeitliche Live-Änderungen
      if (error instanceof ApiError && error.code === 'acceptance-missing' && status === 'done') setAskAcceptance(id);
      else setMessage({ kind: 'error', text: (error as Error).message });
    }
  };

  if (board.unavailable) {
    return (
      <div className="page">
        <DbUnavailable pid={pid} message={board.unavailable} onRetry={board.reloadAll} />
      </div>
    );
  }
  if (board.active.state.status === 'error') {
    return (
      <div className="page">
        <ErrorState message={board.active.state.message} onRetry={board.reloadAll} />
      </div>
    );
  }

  return (
    <div
      className="page db-board"
      onClick={onBoardClick}
      onDragStartCapture={() => {
        dragged.current = true;
      }}
      onDragEndCapture={() => {
        dragged.current = false;
      }}
      onDropCapture={() => {
        setTimeout(() => {
          dragged.current = false;
        }, 0);
      }}
    >
      <div className="db-board-head">
        <h1 className="page-title">Board</h1>
        <label className="db-search">
          <SearchIcon />
          <input type="search" aria-label="Suche im Board" placeholder="Board durchsuchen (ID, Titel, Label)" value={query} onChange={(e) => setQuery(e.target.value)} />
        </label>
        <button type="button" className="btn" disabled={!view || !showsActive} onClick={() => setDialog('specs')}>
          Specs
        </button>
        <button type="button" className="btn" disabled={!view} onClick={() => {
          setNotesBoard(null);
          setDialog('notes');
        }}>
          {openNotes > 0 ? `Notizen (${openNotes})` : 'Notizen'}
        </button>
        <button type="button" className="btn" disabled={!view || !showsActive} onClick={() => openGraph()}>
          Abhängigkeiten
        </button>
        <DispatchIndicator status={dispatch.status} error={dispatch.error} onOpen={() => setDialog('dispatch')} />
      </div>
      {message && (
        <div className={`banner${message.kind === 'info' ? ' info' : ''}`} role={message.kind === 'error' ? 'alert' : 'status'}>
          {message.text}
        </div>
      )}
      {active && active.boardId === null && showsActive && (
        <div className="state empty">
          <h2>Kein Board gewählt</h2>
          <p>Board in der Seitenleiste auswählen oder über den Stift daneben ein Projekt und ein Board anlegen.</p>
        </div>
      )}
      {!showsActive && (
        <div className="state empty">
          <h2>Noch kein Board in {shownProject}</h2>
          <p>Über den Stift neben der Board-Auswahl ein Board anlegen.</p>
        </div>
      )}
      {showsActive && active?.boardId != null && board.view.state.status === 'loading' && <LoadingState text="Board wird geladen …" />}
      {board.view.state.status === 'error' && <ErrorState message={board.view.state.message} onRetry={board.view.retry} />}
      {view && showsActive && (
        <div className="columns db-columns" style={{ '--db-column-count': shownColumns.length } as CSSProperties}>
          {shownColumns.map((column) => {
            const shown = filterDbCards(column.tickets, query);
            const done =
              column.id === 'done'
                ? buildDoneLayout(shown, { grouping: settings?.doneGrouping ?? 'time', archiveDays: settings?.doneArchiveDays ?? 0, showArchived: false, searching, now })
                : null;
            return (
              <DbColumn
                key={column.id}
                column={column}
                cards={shown}
                count={done?.shown}
                onMove={(id, status) => void move(id, status)}
                onOpen={openTicket}
                onLockedDrop={() => setMessage({ kind: 'info', text: WAITING_HINT })}
                agentPrefix={dispatch.status?.agentPrefix}
                compact={compactCards}
                merges={merges}
                activeRuns={activeRuns}
              >
                {done && <DbDoneSections layout={done} expandAll={searching} onOpen={openTicket} merges={merges} now={now} />}
              </DbColumn>
            );
          })}
        </div>
      )}
      <DbAddFab disabled={!view} importDisabled={!showsActive || sources.length === 0} onNew={() => setDialog('new')} onImport={() => setDialog('import')} />
      {dialog === 'new' && view && (
        <DbNewTicketDialog
          boardId={view.boardId}
          onClose={() => setDialog(null)}
          onCreated={(ticket) => {
            setDialog(null);
            board.view.reload();
            setMessage({ kind: 'info', text: `${ticket.id} angelegt.` });
          }}
        />
      )}
      {dialog === 'import' && view && active?.board && (
        <DbImportDialog
          board={active.board}
          boards={boards}
          onClose={() => setDialog(null)}
          onImported={(count) => {
            setDialog(null);
            board.view.reload();
            setMessage({ kind: 'info', text: count === 1 ? '1 Ticket geholt.' : `${count} Tickets geholt.` });
          }}
        />
      )}
      {dialog === 'notes' && (notesBoard ?? view?.boardId) !== undefined && (
        <DbNotesDialog
          boardId={(notesBoard ?? view?.boardId)!}
          onClose={() => {
            setDialog(null);
            setNotesBoard(null);
          }}
          onChanged={() => {
            board.view.reload();
            board.notes.reload();
          }}
          onOpenTicket={(id) => {
            setDialog(null);
            openTicket(id);
          }}
        />
      )}
      {dialog === 'specs' && view && (
        <DbSpecsDialog boardId={view.boardId} onClose={() => setDialog(null)} onOpenSpec={(spec) => openGraph(spec)} />
      )}
      {dialog === 'dispatch' && (
        <DispatchDialog
          boardId={showsActive ? active?.boardId ?? null : null}
          status={dispatch.status}
          error={dispatch.error}
          onReload={() => void dispatch.reload()}
          pid={pid}
          onClose={() => setDialog(null)}
          onStatus={dispatch.set}
          onOpenTicket={(id) => {
            setDialog(null);
            openTicket(id);
          }}
          onChanged={board.view.reload}
        />
      )}
      {dialog === 'graph' && graph && (
        <DbGraphDialog
          scopes={graph.scopes}
          initial={graph.initial}
          onClose={() => setDialog(null)}
          onOpenTicket={(id) => {
            setDialog(null);
            openTicket(id);
          }}
        />
      )}
      {ticketId && view && (
        <DbTicketPanel
          key={ticketId}
          ticketId={ticketId}
          boardTickets={cards}
          columns={view.columns.map(({ id, title }) => ({ id, title }))}
          onClose={closeTicket}
          onChanged={board.view.reload}
          onOpenTicket={openTicket}
          isOwnBoard={isOwnBoard}
          guard={panelGuard}
          agentPrefix={dispatch.status?.agentPrefix}
          userName={active?.user.name}
        />
      )}
      {askAcceptance && (
        <DbAcceptanceDialog
          key={askAcceptance}
          ticketId={askAcceptance}
          onCancel={() => setAskAcceptance(null)}
          onConfirm={(acceptance) => {
            const id = askAcceptance;
            setAskAcceptance(null);
            void move(id, 'done', acceptance);
          }}
        />
      )}
    </div>
  );
}
