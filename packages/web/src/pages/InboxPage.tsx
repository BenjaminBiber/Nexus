import { useRef, useState } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import { INBOX_KIND_LABELS, INBOX_KINDS, type InboxItemDto, type InboxKind, type InboxListDto } from '../api/inboxTypes.js';
import { DbUnavailable } from '../components/db/DbUnavailable.js';
import { EmptyState } from '../components/EmptyState.js';
import { ErrorState } from '../components/ErrorState.js';
import { InboxRow } from '../components/inbox/InboxRow.js';
import { LoadingState } from '../components/LoadingState.js';
import { useAsync } from '../hooks/useAsync.js';
import { useInbox } from '../inbox/useInbox.js';
import { useReorder } from '../inbox/useReorder.js';
import { useDbProjectWatch } from '../live/useLive.js';

// Beobachtet die Fragen eines Projekts (eine Komponente je Projekt der eigenen Boards)
function ProjectWatch({ projectId }: { projectId: number }) {
  useDbProjectWatch(projectId);
  return null;
}

// Arten aus ?kind= (kommagetrennt, unbekannte fallen weg)
function kindsOf(raw: string | null): InboxKind[] {
  return (raw ?? '').split(',').filter((k): k is InboxKind => (INBOX_KINDS as readonly string[]).includes(k));
}

// Seite „Meine Aufgaben“: was auf den eigenen Boards gerade auf mich wartet, in meiner Reihenfolge (neue Einträge oben).
// Filter nach Board (?board=) und Art (?kind=); umsortieren per Ziehen oder mit den Pfeilknöpfen, nur ohne Filter.
export function InboxPage() {
  const { pid = '' } = useParams();
  const api = useApi();
  const [search, setSearch] = useSearchParams();
  const boardParam = Number(search.get('board'));
  const boardId = Number.isInteger(boardParam) && boardParam > 0 ? boardParam : null;
  const kinds = kindsOf(search.get('kind'));
  const inbox = useInbox(boardId);
  // Eigene Boards für die Auswahl und die Fragen-Beobachtung
  const own = useAsync(async () => {
    const [active, boards] = await Promise.all([api.getDbActive(), api.getDbBoards()]);
    return boards.filter((b) => b.ownerId === active.user.id);
  }, [api]);
  const ownBoards = own.state.status === 'ready' ? own.state.data : [];
  const projectIds = [...new Set(ownBoards.map((b) => b.projectId))];
  const [resetError, setResetError] = useState<string | null>(null);
  const dragFrom = useRef<number | null>(null);

  const data = inbox.state.status === 'ready' ? inbox.state.data : null;
  const items = data?.items ?? [];
  const shown = kinds.length ? items.filter((i) => kinds.includes(i.kind)) : items;
  const filtering = boardId !== null || kinds.length > 0;
  const apply = (next: InboxItemDto[]) => data && inbox.set({ ...data, items: next } satisfies InboxListDto);
  const reorder = useReorder(items, apply);

  const setParam = (key: string, value: string | null) => {
    const next = new URLSearchParams(search);
    if (value) next.set(key, value);
    else next.delete(key);
    setSearch(next, { replace: true });
  };
  const toggleKind = (kind: InboxKind) => {
    const next = kinds.includes(kind) ? kinds.filter((k) => k !== kind) : [...kinds, kind];
    setParam('kind', next.length ? INBOX_KINDS.filter((k) => next.includes(k)).join(',') : null);
  };
  const reset = async () => {
    setResetError(null);
    try {
      await api.resetInboxOrder();
      await inbox.reload();
    } catch (e) {
      setResetError((e as Error).message);
    }
  };

  if (inbox.unavailable) {
    return (
      <div className="page">
        <DbUnavailable pid={pid} message={inbox.unavailable} onRetry={() => void inbox.retry()} />
      </div>
    );
  }

  return (
    <div className="page" aria-label="Meine Aufgaben">
      {projectIds.map((id) => <ProjectWatch key={id} projectId={id} />)}
      <header className="questions-head">
        <h1>Meine Aufgaben</h1>
        <span className="muted">Was auf deinen Boards gerade auf dich wartet. Neues steht oben; die Reihenfolge legst du fest.</span>
      </header>
      <div className="filters inbox-filters">
        <label>
          <span className="sr-only">Board</span>
          <select aria-label="Board" value={boardId ?? ''} onChange={(e) => setParam('board', e.target.value || null)}>
            <option value="">Alle eigenen Boards</option>
            {ownBoards.map((b) => (
              <option key={b.id} value={b.id}>{`${b.projectKey} · ${b.name}`}</option>
            ))}
          </select>
        </label>
        <span className="filter-divider" aria-hidden="true" />
        {INBOX_KINDS.map((kind) => (
          <button key={kind} type="button" className={`fchip${kinds.includes(kind) ? ' on' : ''}`} aria-pressed={kinds.includes(kind)} onClick={() => toggleKind(kind)}>
            {INBOX_KIND_LABELS[kind]}
            {data && <span className="mono tab-count">{data.counts[kind]}</span>}
          </button>
        ))}
        <span className="filter-divider" aria-hidden="true" />
        <button type="button" className="btn small" onClick={() => void reset()}>Reihenfolge zurücksetzen</button>
      </div>
      {filtering && shown.length > 1 && <p className="muted inbox-hint">Umsortieren geht nur ohne Filter.</p>}
      {inbox.notice && data && <p role="alert" className="field-error">{inbox.notice}</p>}
      {reorder.error && <p role="alert" className="field-error">{reorder.error}</p>}
      {resetError && <p role="alert" className="field-error">{resetError}</p>}
      {inbox.state.status === 'loading' && <LoadingState text="Aufgaben werden geladen …" />}
      {inbox.state.status === 'error' && <ErrorState message="Aufgaben konnten nicht geladen werden" detail={inbox.state.message} onRetry={inbox.retry} />}
      {data && shown.length === 0 && (
        filtering
          ? <EmptyState title="Nichts mit diesen Filtern" />
          : <EmptyState title="Nichts wartet auf dich">Blockierte Tickets, offene Fragen, Tickets in Planung und offene Notizen deiner Boards erscheinen hier.</EmptyState>
      )}
      {shown.length > 0 && (
        <ol className="inbox-list" aria-label="Aufgaben">
          {shown.map((item, index) => (
            <InboxRow
              key={item.key}
              pid={pid}
              item={item}
              index={index}
              draggable={!filtering}
              onChanged={() => void inbox.reload()}
              onDragStart={(i) => { dragFrom.current = i; }}
              onDrop={(i) => {
                if (dragFrom.current !== null) void reorder.move(dragFrom.current, i);
                dragFrom.current = null;
              }}
            />
          ))}
        </ol>
      )}
    </div>
  );
}
