import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import { useApi } from '../../api/ApiContext.js';
import { DB_PRIORITY_LABELS, type DbBoardDto, type DbBoardViewDto, type DbColumnId } from '../../api/dbTypes.js';
import { DbModal } from './DbModal.js';

// Diese Spalten sind beim Öffnen zugeklappt (meist nichts mehr zu holen)
const COLLAPSED: DbColumnId[] = ['done', 'not_needed'];

// Boards, von denen sich Tickets holen lassen: dasselbe Projekt, nicht das Board selbst
export function importSources(board: DbBoardDto, boards: DbBoardDto[]): DbBoardDto[] {
  return boards.filter((b) => b.projectId === board.projectId && b.id !== board.id);
}

type Loaded = { status: 'loading' } | { status: 'error'; message: string } | { status: 'ready'; view: DbBoardViewDto };

// „Von anderem Board holen“: Quell-Board desselben Projekts wählen, Tickets je Spalte ankreuzen und in einem Schritt
// auf das aktuelle Board holen. Ein Fehler erscheint im Dialog, die Auswahl bleibt.
export function DbImportDialog({ board, boards, onClose, onImported }: { board: DbBoardDto; boards: DbBoardDto[]; onClose: () => void; onImported: (count: number) => void }) {
  const api = useApi();
  const sources = importSources(board, boards);
  const [sourceId, setSourceId] = useState<number | null>(sources[0]?.id ?? null);
  const [loaded, setLoaded] = useState<Loaded>({ status: 'loading' });
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [expanded, setExpanded] = useState<Partial<Record<DbColumnId, boolean>>>({});
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const sending = useRef(false);
  const titleId = useId();

  // Lädt die Ansicht des Quell-Boards; ein späterer Wechsel gewinnt
  useEffect(() => {
    if (sourceId === null) return;
    let current = true;
    setLoaded({ status: 'loading' });
    api.getDbBoardView(sourceId).then(
      (view) => current && setLoaded({ status: 'ready', view }),
      (e: Error) => current && setLoaded({ status: 'error', message: e.message }),
    );
    return () => {
      current = false;
    };
  }, [api, sourceId]);

  const columns = loaded.status === 'ready' ? loaded.view.columns.filter((c) => c.tickets.length > 0) : [];
  // Auswahl in Anzeige-Reihenfolge (Spalten, darin Karten)
  const ordered = columns.flatMap((c) => c.tickets.map((t) => t.id)).filter((id) => selected.has(id));

  // Wechselt das Quell-Board; Auswahl, Klappzustand und Fehler beginnen neu
  const chooseSource = (id: number) => {
    setSourceId(id);
    setSelected(new Set());
    setExpanded({});
    setError(null);
  };

  // Setzt oder entfernt die Auswahl für mehrere Tickets
  const mark = (ids: string[], on: boolean) => {
    setSelected((prev) => {
      const next = new Set(prev);
      for (const id of ids) {
        if (on) next.add(id);
        else next.delete(id);
      }
      return next;
    });
  };

  const isOpen = (id: DbColumnId) => expanded[id] ?? !COLLAPSED.includes(id);

  // Holt die gewählten Tickets; bei Erfolg meldet der Aufrufer und schließt
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (sending.current || ordered.length === 0) return;
    sending.current = true;
    setBusy(true);
    setError(null);
    try {
      const result = await api.importDbTickets(board.id, ordered);
      onImported(result.moved.length);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      sending.current = false;
      setBusy(false);
    }
  };

  const cancel = () => {
    if (!busy) onClose();
  };

  return (
    <DbModal labelledBy={titleId} className="panel db-modal wide" onSubmit={(e) => void submit(e)} onEscape={cancel}>
      <h2 id={titleId}>Von anderem Board holen</h2>
      <label className="field">
        <span>Quell-Board</span>
        <select value={sourceId === null ? '' : String(sourceId)} disabled={busy} onChange={(e) => chooseSource(Number(e.target.value))}>
          {sources.map((b) => (
            <option key={b.id} value={b.id}>
              {b.name} ({b.ownerName})
            </option>
          ))}
        </select>
      </label>
      {loaded.status === 'loading' && sourceId !== null && <p className="muted">Board wird geladen …</p>}
      {loaded.status === 'error' && (
        <p role="alert" className="field-error">
          {loaded.message}
        </p>
      )}
      {loaded.status === 'ready' && columns.length === 0 && <p className="muted">Auf diesem Board liegen keine Tickets.</p>}
      <div className="db-import-columns">
        {columns.map((column) => {
          const open = isOpen(column.id);
          const ids = column.tickets.map((t) => t.id);
          return (
            <section key={column.id} className="db-import-column">
              <div className="db-import-head">
                <button type="button" className="link-btn" aria-expanded={open} onClick={() => setExpanded((prev) => ({ ...prev, [column.id]: !open }))}>
                  <span aria-hidden="true">{open ? '▾' : '▸'}</span> {column.title} ({column.tickets.length})
                </button>
                <button type="button" className="link-btn" aria-label={`Alle in ${column.title} wählen`} onClick={() => mark(ids, true)}>
                  Alle wählen
                </button>
                <button type="button" className="link-btn" aria-label={`Keine in ${column.title}`} onClick={() => mark(ids, false)}>
                  Keine
                </button>
              </div>
              {open && (
                <ul className="db-import-list">
                  {column.tickets.map((t) => (
                    <li key={t.id}>
                      <label className="option-row">
                        <input type="checkbox" checked={selected.has(t.id)} onChange={(e) => mark([t.id], e.target.checked)} />
                        <span className="card-id">{t.id}</span> <span>{t.title}</span>
                        {t.assignee && <span className="badge">@{t.assignee}</span>}
                        {t.priority !== 'normal' && <span className={`badge prio-${t.priority}`}>{DB_PRIORITY_LABELS[t.priority]}</span>}
                      </label>
                    </li>
                  ))}
                </ul>
              )}
            </section>
          );
        })}
      </div>
      {error && (
        <p role="alert" className="field-error">
          {error}
        </p>
      )}
      <div className="actions">
        <button type="submit" className="btn primary" disabled={busy || ordered.length === 0}>
          Importieren ({ordered.length})
        </button>
        <button type="button" className="btn" disabled={busy} onClick={cancel}>
          Abbrechen
        </button>
      </div>
    </DbModal>
  );
}
