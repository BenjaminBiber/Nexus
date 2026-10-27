import { useId, useRef, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import { ApiError } from '../../api/ApiError.js';
import type { DbNoteDto, DbNoteFilter, DbSimilarNote } from '../../api/dbTypes.js';
import { formatDate } from '../../dbboard/formatDate.js';
import { useAsync } from '../../hooks/useAsync.js';
import { MarkdownView } from '../../markdown/MarkdownView.js';
import { DbModal } from './DbModal.js';

const FILTERS: Array<{ key: DbNoteFilter; label: string }> = [
  { key: 'open', label: 'Offen' },
  { key: 'taken', label: 'Übernommen' },
  { key: 'dropped', label: 'Verworfen' },
  { key: 'all', label: 'Alle' },
];

const TITLE_LENGTH = 200;

// Titel der Notiz; ältere Notizen ohne Titel: erste nicht leere Zeile (höchstens 200 Zeichen)
function noteTitle(note: DbNoteDto): string {
  return (note.title.trim() || (note.body.split('\n').map((l) => l.trim()).find(Boolean) ?? '')).slice(0, TITLE_LENGTH);
}

// Kandidaten aus der Ablehnung „note-similar“; null bei jedem anderen Fehler
function similarFrom(e: unknown): DbSimilarNote[] | null {
  if (!(e instanceof ApiError) || e.code !== 'note-similar') return null;
  const notes = e.details?.notes;
  return Array.isArray(notes) ? (notes as DbSimilarNote[]) : [];
}

// Kommagetrennte Eingabe als Liste
function split(text: string): string[] {
  return text.split(',').map((s) => s.trim()).filter(Boolean);
}

// Notizen des Boards: filtern, anlegen, als Ticket übernehmen, mit Grund verwerfen. Weg ist nichts.
// Mit ungespeicherten Eingaben fragen Schließen, Esc und Ticket-Verweise erst nach.
export function DbNotesDialog({ boardId, onClose, onChanged, onOpenTicket }: {
  boardId: number;
  onClose: () => void;
  onChanged: () => void;
  onOpenTicket: (id: string) => void;
}) {
  const api = useApi();
  const [filter, setFilter] = useState<DbNoteFilter>('open');
  const notes = useAsync(() => api.getDbNotes(boardId, filter), [api, boardId, filter]);
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [labels, setLabels] = useState('');
  // Ähnliche Notizen, mit denen das Anlegen abgelehnt wurde: hochzählen oder trotzdem anlegen
  const [similar, setSimilar] = useState<DbSimilarNote[] | null>(null);
  // Offenes Übernahme-Formular; `initial` ist der Titelvorschlag (geändert = ungespeicherte Eingabe)
  const [taking, setTaking] = useState<{ id: number; title: string; initial: string } | null>(null);
  const [dropping, setDropping] = useState<{ id: number; reason: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [leaving, setLeaving] = useState<(() => void) | null>(null);
  const sending = useRef(false);
  const titleRef = useRef<HTMLInputElement>(null);
  const dialogRef = useRef<HTMLElement | null>(null);
  const titleId = useId();
  // Beim Filterwechsel bleibt die bisherige Liste stehen, bis die neue da ist (kein leeres Aufblitzen)
  const [shown, setShown] = useState<DbNoteDto[]>([]);
  if (notes.state.status === 'ready' && notes.state.data !== shown) setShown(notes.state.data);
  const list = notes.state.status === 'ready' ? notes.state.data : notes.state.status === 'loading' ? shown : [];
  const dirty =
    title.trim() !== '' ||
    body.trim() !== '' ||
    labels.trim() !== '' ||
    (dropping?.reason.trim() ?? '') !== '' ||
    (taking !== null && taking.title.trim() !== taking.initial.trim());
  // Höchstens eine Fehlermeldung zugleich: Aktion und Laden zusammen
  const alert = [error, notes.state.status === 'error' ? notes.state.message : null].filter(Boolean).join(' ');

  // Verlassen (Schließen, Ticket öffnen); mit Eingaben erst nachfragen
  const leave = (action: () => void) => {
    if (dirty) setLeaving(() => action);
    else action();
  };

  // Führt eine Aktion aus, lädt nach und meldet die Änderung; true bei Erfolg. Nie zwei zugleich.
  // Lehnt der Server eine Notiz als schon vorhanden ab, erscheinen die Kandidaten statt einer Fehlermeldung.
  const run = async (work: () => Promise<unknown>): Promise<boolean> => {
    if (sending.current) return false;
    sending.current = true;
    setBusy(true);
    setError(null);
    try {
      await work();
      notes.reload();
      onChanged();
      return true;
    } catch (e) {
      const found = similarFrom(e);
      if (found) setSimilar(found);
      else setError((e as Error).message);
      return false;
    } finally {
      sending.current = false;
      setBusy(false);
    }
  };

  // Leert die Eingabe nach Anlegen oder Hochzählen
  const reset = () => {
    setTitle('');
    setBody('');
    setLabels('');
    setSimilar(null);
    titleRef.current?.focus(); // gleich die nächste Notiz schreiben
  };

  // Legt eine Notiz an; force auch dann, wenn es eine ähnliche gibt
  const add = async (force = false) => {
    if (!title.trim()) return;
    if (await run(() => api.addDbNote(boardId, { title: title.trim(), body, labels: split(labels), force }))) reset();
  };

  // Derselbe Befund steht schon in einer offenen Notiz: diese hochzählen statt neu anlegen
  const countSeen = async (id: number) => {
    if (await run(() => api.seenDbNote(id))) reset();
  };

  // Macht aus der gewählten Notiz ein Ticket
  const take = async () => {
    if (!taking) return;
    if (!taking.title.trim()) return setError('Der Titel darf nicht leer sein.');
    if (await run(() => api.takeDbNote(taking.id, { title: taking.title.trim() }))) setTaking(null);
  };

  // Verwirft die gewählte Notiz; ohne Grund nicht
  const drop = async () => {
    if (!dropping) return;
    if (!dropping.reason.trim()) return setError('Ohne Grund wird nichts verworfen.');
    if (await run(() => api.dropDbNote(dropping.id, dropping.reason.trim()))) setDropping(null);
  };

  const open = (id: string) => leave(() => onOpenTicket(id));

  // Schließt ein offenes Übernahme- oder Verwerfen-Formular; false, wenn keines offen war
  const cancelInline = (): boolean => {
    if (!taking && !dropping) return false;
    setTaking(null);
    setDropping(null);
    setError(null);
    return true;
  };

  return (
    <DbModal
      labelledBy={titleId}
      dialogRef={dialogRef}
      className="panel db-modal wide"
      onEscape={() => {
        if (leaving) setLeaving(null);
        else if (!cancelInline()) leave(onClose);
      }}
      onBackdrop={() => {
        if (!leaving) leave(onClose);
      }}
    >
      <div className="panel-head">
        <h2 id={titleId}>Notizen</h2>
        <button type="button" className="icon-btn" aria-label="Notizen schließen" autoFocus onClick={() => leave(onClose)}>
          ×
        </button>
      </div>
      {leaving && (
        <div role="alertdialog" aria-label="Eingaben verwerfen" className="actions">
          <span>Eingaben verwerfen?</span>
          <button type="button" className="btn danger" onClick={() => {
            const action = leaving;
            setLeaving(null);
            action();
          }}>
            Verwerfen
          </button>
          <button type="button" className="btn" autoFocus onClick={() => setLeaving(null)}>
            Weiter bearbeiten
          </button>
        </div>
      )}
      <div className="chips" role="group" aria-label="Filter">
        {FILTERS.map((f) => (
          <button key={f.key} type="button" className={`fchip${filter === f.key ? ' on' : ''}`} aria-pressed={filter === f.key} onClick={() => setFilter(f.key)}>
            {f.label}
          </button>
        ))}
      </div>
      <label className="field">
        <span>Neue Notiz</span>
        <input
          ref={titleRef}
          value={title}
          maxLength={TITLE_LENGTH}
          placeholder="Was fällt auf? Ein Satz, der ohne Kontext verständlich ist"
          onChange={(e) => {
            setTitle(e.target.value);
            setSimilar(null);
          }}
        />
      </label>
      <label className="field">
        <span>Details</span>
        <textarea
          className="plain-text"
          rows={3}
          value={body}
          placeholder="Wo, woran man es sieht, vermutete Ursache, Vorschlag"
          onChange={(e) => {
            setBody(e.target.value);
            setSimilar(null);
          }}
        />
      </label>
      <label className="field">
        <span>Labels der Notiz</span>
        <input value={labels} placeholder="kommagetrennt" onChange={(e) => setLabels(e.target.value)} />
      </label>
      {similar ? (
        <div role="alert" className="field-error">
          <p>Diesen Befund gibt es vermutlich schon:</p>
          <ul>
            {similar.map((s) => (
              <li key={s.id}>
                {s.title}{' '}
                {s.state === 'open' ? (
                  <button type="button" className="btn" disabled={busy} onClick={() => void countSeen(s.id)}>
                    Ist derselbe – hochzählen
                  </button>
                ) : (
                  s.ticketId && (
                    <button type="button" className="link-btn" onClick={() => open(s.ticketId!)}>
                      schon als {s.ticketId}
                    </button>
                  )
                )}
              </li>
            ))}
          </ul>
          <div className="actions">
            <button type="button" className="btn" disabled={busy} onClick={() => void add(true)}>
              Trotzdem anlegen
            </button>
          </div>
        </div>
      ) : (
        <div className="actions">
          <button type="button" className="btn primary" disabled={!title.trim() || busy} onClick={() => void add()}>
            Notiz anlegen
          </button>
        </div>
      )}
      {alert && (
        <p role="alert" className="field-error">
          {alert}
        </p>
      )}
      {notes.state.status === 'ready' && list.length === 0 && (
        <p className="muted">
          {filter === 'open' ? 'Keine offenen Notizen. Was beim Arbeiten auffällt und nicht ins laufende Ticket gehört, kommt hier hinein.' : 'Nichts mit diesem Stand.'}
        </p>
      )}
      <ul className="db-notes" aria-busy={notes.state.status === 'loading'}>
        {list.map((note) => (
          <li key={note.id} className="db-note">
            <div className="entry-meta">
              {note.state === 'open' && <span className="badge waiting">offen</span>}
              {note.state === 'taken' && note.ticketId && (
                <span className="badge running">
                  übernommen als{' '}
                  <button type="button" className="link-btn" onClick={() => open(note.ticketId!)}>
                    {note.ticketId}
                  </button>
                </span>
              )}
              {note.state === 'dropped' && <span className="badge">verworfen: {note.reason}</span>}
              {note.source && (
                <button type="button" className="link-btn" onClick={() => open(note.source!)}>
                  aus {note.source}
                </button>
              )}
              {note.seenCount > 1 && (
                <span className="badge" title={note.seenIn.length ? `auch bei ${note.seenIn.join(', ')}` : undefined}>
                  {note.seenCount}× aufgefallen
                </span>
              )}
              {note.labels.map((label) => (
                <span key={label} className="tag">
                  {label}
                </span>
              ))}
              <span className="muted">
                {note.authorName}, {formatDate(note.createdAt)}
              </span>
            </div>
            {note.title && <strong className="db-note-title">{note.title}</strong>}
            {note.body.trim() && <MarkdownView source={note.body} />}
            {note.state === 'open' && taking?.id === note.id && (
              <div className="actions">
                <label className="field">
                  <span>Titel des Tickets</span>
                  <input value={taking.title} autoFocus onChange={(e) => setTaking({ ...taking, title: e.target.value })} />
                </label>
                <button type="button" className="btn primary" disabled={busy} onClick={() => void take()}>
                  Ticket anlegen
                </button>
                <button type="button" className="btn" disabled={busy} onClick={() => setTaking(null)}>
                  Abbrechen
                </button>
              </div>
            )}
            {note.state === 'open' && dropping?.id === note.id && (
              <div className="actions">
                <label className="field">
                  <span>Grund</span>
                  <input value={dropping.reason} autoFocus onChange={(e) => setDropping({ id: note.id, reason: e.target.value })} />
                </label>
                <button type="button" className="btn danger" disabled={busy} onClick={() => void drop()}>
                  Verwerfen bestätigen
                </button>
                <button type="button" className="btn" disabled={busy} onClick={() => setDropping(null)}>
                  Abbrechen
                </button>
              </div>
            )}
            {note.state === 'open' && taking?.id !== note.id && dropping?.id !== note.id && (
              <div className="actions">
                <button type="button" className="btn primary" onClick={() => {
                  setDropping(null);
                  const suggestion = noteTitle(note);
                  setTaking({ id: note.id, title: suggestion, initial: suggestion });
                }}>
                  Ticket daraus machen
                </button>
                <button type="button" className="btn" onClick={() => {
                  setTaking(null);
                  setDropping({ id: note.id, reason: '' });
                }}>
                  Verwerfen
                </button>
              </div>
            )}
          </li>
        ))}
      </ul>
    </DbModal>
  );
}
