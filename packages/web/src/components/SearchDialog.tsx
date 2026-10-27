import { useId, useRef, useState, type KeyboardEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  DB_COLUMN_LABELS, DB_QUESTION_STATE_LABELS, type DbColumnId, type DbSearchField, type DbSearchHitDto, type DbSearchKind,
} from '../api/dbTypes.js';
import { useGlobalSearch } from '../hooks/useGlobalSearch.js';
import { DbModal } from './db/DbModal.js';
import { Highlight } from './Highlight.js';

const KIND_TITLES: Record<DbSearchKind, string> = { ticket: 'Tickets', question: 'Fragen', note: 'Notizen' };
const FIELD_LABELS: Record<DbSearchField, string> = {
  title: 'Titel', description: 'Beschreibung', comment: 'Kommentar', plan: 'Plan', handbook: 'Handbuch', acceptance: 'Abnahme',
  decision: 'Entscheidung', 'question-comment': 'Kommentar', note: 'Notiz',
};
const NOTE_STATES: Record<string, string> = { open: 'offen', taken: 'übernommen', dropped: 'verworfen' };

// Zustand eines Treffers in deutscher Schreibweise
function statusLabel(hit: DbSearchHitDto): string {
  if (hit.kind === 'ticket') return DB_COLUMN_LABELS[hit.status as DbColumnId] ?? hit.status;
  if (hit.kind === 'question') return (DB_QUESTION_STATE_LABELS as Record<string, string>)[hit.status] ?? hit.status;
  return NOTE_STATES[hit.status] ?? hit.status;
}

// Ziel eines Treffers: Ticket-Panel, Fragenseite oder Notizen-Dialog des Boards
export function searchTarget(pid: string, hit: DbSearchHitDto): { to: string; state?: unknown } {
  const p = encodeURIComponent(pid);
  if (hit.ticketId) return { to: `/p/${p}/board/t/${encodeURIComponent(hit.ticketId)}` };
  if (hit.kind === 'question') return { to: `/p/${p}/questions/${encodeURIComponent(hit.id)}` };
  return { to: `/p/${p}/board`, state: hit.board ? { notesBoard: hit.board.id } : undefined };
}

// Such-Dialog über alle Boards (Strg+K): Eingabe mit Verzögerung, Treffer nach Art gruppiert mit Projekt/Board, Status und
// hervorgehobener Fundstelle; ↑/↓ wählt, Enter öffnet, Esc schließt.
export function SearchDialog({ pid, onClose }: { pid: string; onClose: () => void }) {
  const navigate = useNavigate();
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const state = useGlobalSearch(query);
  const inputRef = useRef<HTMLInputElement>(null);
  const id = useId();
  const result = state.status === 'ready' ? state.result : state.status === 'loading' ? state.previous : null;
  const hits = result?.hits ?? [];
  const current = Math.min(active, Math.max(hits.length - 1, 0));

  const open = (hit: DbSearchHitDto) => {
    const target = searchTarget(pid, hit);
    onClose();
    navigate(target.to, target.state ? { state: target.state } : undefined);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown' && hits.length) {
      event.preventDefault();
      setActive((current + 1) % hits.length);
    } else if (event.key === 'ArrowUp' && hits.length) {
      event.preventDefault();
      setActive((current - 1 + hits.length) % hits.length);
    } else if (event.key === 'Enter' && hits[current]) {
      event.preventDefault();
      open(hits[current]);
    }
  };

  const groups = (['ticket', 'question', 'note'] as DbSearchKind[])
    .map((kind) => ({ kind, items: hits.map((hit, index) => ({ hit, index })).filter(({ hit }) => hit.kind === kind) }))
    .filter((g) => g.items.length);
  const optionId = (index: number) => `${id}-hit-${index}`;

  return (
    <DbModal labelledBy={`${id}-title`} onEscape={onClose} onBackdrop={onClose} className="panel db-modal search-dialog" initialFocus={inputRef}>
      <h2 id={`${id}-title`} className="sr-only">Suche über alle Boards</h2>
      <label className="search search-dialog-input">
        <span className="sr-only">Suchbegriff</span>
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" aria-hidden="true">
          <circle cx="7" cy="7" r="4.5" />
          <path d="M10.5 10.5L14 14" />
        </svg>
        <input
          ref={inputRef}
          type="search"
          role="combobox"
          aria-expanded={hits.length > 0}
          aria-controls={`${id}-list`}
          aria-autocomplete="list"
          aria-activedescendant={hits.length ? optionId(current) : undefined}
          placeholder="Tickets, Kommentare, Pläne, Fragen und Notizen durchsuchen"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={onKeyDown}
        />
      </label>
      <div className="search-dialog-body">
        {state.status === 'idle' && <p className="muted">Mindestens zwei Zeichen eingeben. ↑/↓ wählen, Enter öffnet, Esc schließt.</p>}
        {state.status === 'error' && <p role="alert" className="field-error">{state.message}</p>}
        {state.status === 'loading' && !result && <p className="muted">Suche läuft …</p>}
        {state.status === 'ready' && hits.length === 0 && <p className="muted">Keine Treffer.</p>}
        <div role="listbox" id={`${id}-list`} aria-label="Treffer" className="search-hits" hidden={!hits.length}>
          {groups.map((g) => (
            <div role="group" aria-label={KIND_TITLES[g.kind]} key={g.kind} className="search-group">
              <div className="search-group-title" aria-hidden="true">{KIND_TITLES[g.kind]}</div>
              {g.items.map(({ hit, index }) => (
                <div
                  key={`${hit.kind}:${hit.id}`}
                  id={optionId(index)}
                  role="option"
                  aria-selected={index === current}
                  className={`search-hit${index === current ? ' active' : ''}`}
                  onMouseMove={() => setActive(index)}
                  onClick={() => open(hit)}
                >
                  <div className="search-hit-head">
                    <span className="mono">{hit.kind === 'note' ? `Notiz ${hit.id}` : hit.id}</span>
                    <strong>{hit.title}</strong>
                  </div>
                  <div className="search-hit-meta muted">
                    {hit.project.key}
                    {hit.board ? ` · ${hit.board.name}` : ''} · {statusLabel(hit)} · {FIELD_LABELS[hit.field]}
                  </div>
                  <div className="search-hit-snippet">
                    <Highlight text={hit.snippet} ranges={hit.matches} />
                  </div>
                </div>
              ))}
            </div>
          ))}
        </div>
        {result?.truncated && <p className="muted">Es gibt weitere Treffer – Suche genauer eingrenzen.</p>}
      </div>
    </DbModal>
  );
}
