import { useEffect, useRef, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { DbNoteDto } from '../../api/dbTypes.js';
import { formatDate } from '../../dbboard/formatDate.js';
import { useAsync } from '../../hooks/useAsync.js';
import { useRestoreFocus } from '../../hooks/useRestoreFocus.js';
import { MarkdownView } from '../../markdown/MarkdownView.js';

// Titelvorschlag für das Ticket: Titel der Notiz, sonst ihre erste nicht leere Zeile
function suggestion(note: DbNoteDto): string {
  return (note.title.trim() || (note.body.split('\n').map((l) => l.trim()).find(Boolean) ?? '')).slice(0, 200);
}

// Slide-In von rechts mit einer Notiz aus „Was muss ich tun?“: Inhalt, Herkunft, Labels; offen auch als Ticket
// übernehmen oder mit Grund verwerfen. Esc oder × schließt, Verweise auf Tickets öffnen sie auf dem Board.
export function NoteSlideIn({ noteId, boardId, onClose, onChanged, onOpenTicket }: {
  noteId: number;
  boardId: number;
  onClose: () => void;
  onChanged: () => void;
  onOpenTicket: (id: string) => void;
}) {
  const api = useApi();
  // Eine einzelne Notiz gibt es nur über die Liste des Boards
  const notes = useAsync(() => api.getDbNotes(boardId, 'all'), [api, boardId]);
  const note = notes.state.status === 'ready' ? notes.state.data.find((n) => n.id === noteId) ?? null : null;
  const [mode, setMode] = useState<{ kind: 'take' | 'drop'; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const panelRef = useRef<HTMLElement | null>(null);
  useRestoreFocus(panelRef);
  useEffect(() => {
    panelRef.current?.focus();
  }, []);

  const run = async (work: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await work();
      setMode(null);
      notes.reload();
      onChanged();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const submit = () => {
    if (!mode) return;
    const text = mode.text.trim();
    if (!text) return setError(mode.kind === 'take' ? 'Der Titel darf nicht leer sein.' : 'Ohne Grund wird nichts verworfen.');
    void run(() => (mode.kind === 'take' ? api.takeDbNote(noteId, { title: text }) : api.dropDbNote(noteId, text)));
  };

  return (
    <aside
      ref={panelRef}
      tabIndex={-1}
      className="note-slide"
      role="dialog"
      aria-modal="false"
      aria-label={`Notiz N-${noteId}`}
      onKeyDown={(e) => {
        if (e.key !== 'Escape') return;
        e.stopPropagation();
        if (mode) setMode(null);
        else onClose();
      }}
    >
      <header className="note-slide-head">
        <button type="button" className="icon-btn" aria-label="Notiz schließen" onClick={onClose}>×</button>
        <div className="note-slide-heading">
          <span className="mono muted">N-{noteId}</span>
          <h2>{note ? note.title || suggestion(note) : 'Notiz'}</h2>
        </div>
      </header>
      <div className="note-slide-body">
        {notes.state.status === 'loading' && <p className="muted">Notiz wird geladen …</p>}
        {notes.state.status === 'error' && <p role="alert" className="field-error">Notiz konnte nicht geladen werden: {notes.state.message}</p>}
        {notes.state.status === 'ready' && !note && <p className="muted">Die Notiz gibt es auf diesem Board nicht mehr.</p>}
        {note && (
          <>
            <div className="entry-meta">
              {note.state === 'open' && <span className="badge waiting">offen</span>}
              {note.state === 'taken' && note.ticketId && (
                <span className="badge running">
                  übernommen als <button type="button" className="link-btn" onClick={() => onOpenTicket(note.ticketId!)}>{note.ticketId}</button>
                </span>
              )}
              {note.state === 'dropped' && <span className="badge">verworfen: {note.reason}</span>}
              {note.source && <button type="button" className="link-btn" onClick={() => onOpenTicket(note.source!)}>aus {note.source}</button>}
              {note.seenCount > 1 && (
                <span className="badge" title={note.seenIn.length ? `auch bei ${note.seenIn.join(', ')}` : undefined}>{note.seenCount}× aufgefallen</span>
              )}
              {note.labels.map((label) => <span key={label} className="tag">{label}</span>)}
              <span className="muted">{note.authorName}, {formatDate(note.createdAt)}</span>
            </div>
            {note.body.trim() ? <MarkdownView source={note.body} /> : <p className="muted">Keine Details.</p>}
          </>
        )}
      </div>
      {note?.state === 'open' && (
        <footer className="note-slide-foot">
          {error && <p role="alert" className="field-error">{error}</p>}
          {mode ? (
            <div className="actions">
              <label className="field">
                <span>{mode.kind === 'take' ? 'Titel des Tickets' : 'Grund'}</span>
                <input
                  value={mode.text} autoFocus
                  onChange={(e) => setMode({ ...mode, text: e.target.value })}
                  onKeyDown={(e) => e.key === 'Enter' && submit()}
                />
              </label>
              <button type="button" className={`btn ${mode.kind === 'take' ? 'primary' : 'danger'}`} disabled={busy} onClick={submit}>
                {mode.kind === 'take' ? 'Ticket anlegen' : 'Verwerfen bestätigen'}
              </button>
              <button type="button" className="btn" disabled={busy} onClick={() => { setMode(null); setError(null); }}>Abbrechen</button>
            </div>
          ) : (
            <div className="actions">
              <button type="button" className="btn primary" onClick={() => setMode({ kind: 'take', text: suggestion(note) })}>Ticket daraus machen</button>
              <button type="button" className="btn" onClick={() => setMode({ kind: 'drop', text: '' })}>Verwerfen</button>
            </div>
          )}
        </footer>
      )}
    </aside>
  );
}
