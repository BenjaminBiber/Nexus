import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useApi } from '../../api/ApiContext.js';
import type { InboxItemDto, InboxKind } from '../../api/inboxTypes.js';
import { useDispatchAction } from '../../dbboard/useDispatchAction.js';
import { useInbox } from '../../inbox/useInbox.js';
import { inboxTarget } from '../inbox/InboxRow.js';
import { NoteSlideIn } from './NoteSlideIn.js';

/** Ab diesem Alter gilt ein blockierter Eintrag als dringend */
const URGENT_MS = 24 * 3_600_000;

type GroupKey = 'blocked' | 'question' | 'accept' | 'note';

const GROUPS: Array<{ key: GroupKey; label: string; kinds: InboxKind[] }> = [
  { key: 'blocked', label: 'Blockierte', kinds: ['merge', 'setup', 'blocked'] },
  { key: 'question', label: 'Fragen', kinds: ['question'] },
  { key: 'accept', label: 'Freigaben', kinds: ['accept', 'planning'] },
  { key: 'note', label: 'Notizen', kinds: ['note'] },
];

// Wartezeit kurz: „40 min“, „5 Std.“, „3 Tage“
export function waitText(since: string, now: Date): string {
  const hours = Math.max(0, now.getTime() - Date.parse(since)) / 3_600_000;
  if (hours < 1) return `${Math.round(hours * 60)} min`;
  if (hours < 24) return `${Math.round(hours)} Std.`;
  const days = Math.round(hours / 24);
  return `${days} ${days === 1 ? 'Tag' : 'Tage'}`;
}

// Rechte Spalte „Was muss ich tun?“: Einträge von „Meine Aufgaben“ in vier aufklappbaren Gruppen, mit Aktion je Art.
// `boardId`: eigenes Board des Filters, sonst alle eigenen Boards. Ohne Board-Datenbank bleibt die Spalte leer.
export function TodoColumn({ pid, boardId, now = new Date() }: { pid: string; boardId: number | null; now?: Date }) {
  const inbox = useInbox(boardId);
  // Von Hand auf- oder zugeklappte Gruppen; sonst sind Gruppen mit Einträgen offen und leere zu
  const [toggled, setToggled] = useState<Partial<Record<GroupKey, boolean>>>({});
  const items = inbox.state.status === 'ready' ? inbox.state.data.items : [];
  const toggle = (key: GroupKey, open: boolean) => setToggled((t) => ({ ...t, [key]: !open }));
  // Notiz im Slide-In statt Sprung aufs Board
  const [note, setNote] = useState<{ id: number; boardId: number } | null>(null);
  const navigate = useNavigate();
  const openNote = (item: InboxItemDto) => item.noteId !== null && item.boardId !== null && setNote({ id: item.noteId, boardId: item.boardId });
  return (
    <aside className="todo-column" aria-label="Was muss ich tun?">
      <div className="todo-scroll">
        {inbox.state.status === 'loading' && <p className="muted todo-note">Aufgaben werden geladen …</p>}
        {inbox.state.status === 'error' && !inbox.unavailable && <p className="field-error todo-note" role="alert">Aufgaben konnten nicht geladen werden: {inbox.state.message}</p>}
        {inbox.notice && <p className="muted todo-note">{inbox.notice}</p>}
        {inbox.state.status === 'ready' && items.length === 0 ? (
          <p className="todo-done">Nichts zu tun – der Dispatcher kommt ohne dich weiter.</p>
        ) : inbox.state.status === 'ready' && GROUPS.map((g) => {
          const rows = items.filter((i) => g.kinds.includes(i.kind))
            .sort((a, b) => (g.key === 'blocked' ? Date.parse(a.since) - Date.parse(b.since) : 0) || a.rank - b.rank);
          const open = toggled[g.key] ?? rows.length > 0;
          const urgent = g.key === 'blocked' ? rows.filter((r) => now.getTime() - Date.parse(r.since) >= URGENT_MS).length : 0;
          return (
            <section key={g.key} className={`todo-group${rows.length ? '' : ' empty'}`}>
              <button type="button" className="todo-head" aria-expanded={open} onClick={() => toggle(g.key, open)}>
                {!open && urgent > 0 && <span className="todo-urgent" title={`${urgent} seit über 24 Std.`}>!</span>}
                <span className="todo-label">{g.label}</span>
                <span className="mono todo-count">{rows.length}</span>
                <span className="todo-chevron" aria-hidden="true">▾</span>
              </button>
              {open && (rows.length ? (
                <ul>
                  {rows.map((item) => (
                    <TodoRow key={item.key} pid={pid} item={item} now={now} flag={g.key === 'blocked'} onChanged={() => void inbox.reload()} onOpenNote={openNote} />
                  ))}
                </ul>
              ) : <p className="todo-none">Nichts offen.</p>)}
            </section>
          );
        })}
      </div>
      {note && (
        <NoteSlideIn
          key={note.id} noteId={note.id} boardId={note.boardId}
          onClose={() => setNote(null)}
          onChanged={() => void inbox.reload()}
          onOpenTicket={(id) => navigate(`/p/${encodeURIComponent(pid)}/board/t/${encodeURIComponent(id)}`)}
        />
      )}
    </aside>
  );
}

// Eine Aufgabe: ID, Titel, „seit … · Detail“ und Aktion (Abnehmen, Erneut mergen, Ticket daraus, sonst Öffnen/Antworten)
function TodoRow({ pid, item, now, flag, onChanged, onOpenNote }: {
  pid: string; item: InboxItemDto; now: Date; flag: boolean; onChanged: () => void; onOpenNote: (item: InboxItemDto) => void;
}) {
  const api = useApi();
  const dispatch = useDispatchAction();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const target = inboxTarget(pid, item);
  const id = item.questionId ?? item.ticketId ?? (item.noteId !== null ? `N-${item.noteId}` : '');
  const urgent = flag && now.getTime() - Date.parse(item.since) >= URGENT_MS;
  const detail = [`seit ${waitText(item.since, now)}`, item.detail ?? (item.kind === 'question' && item.waitingTickets ? `${item.waitingTickets} ${item.waitingTickets === 1 ? 'Ticket wartet' : 'Tickets warten'}` : null)]
    .filter(Boolean).join(' · ');

  const take = async () => {
    if (item.noteId === null) return;
    setBusy(true);
    setError(null);
    try {
      await api.takeDbNote(item.noteId);
      onChanged();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const action = () => {
    if (item.kind === 'accept' && item.ticketId) {
      return <button type="button" className="todo-action" disabled={dispatch.busy} onClick={() => void dispatch.run('accept', item.ticketId!, item.boardId ?? undefined, onChanged)}>Abnehmen</button>;
    }
    if (item.kind === 'merge' && item.ticketId) {
      return <button type="button" className="todo-action" disabled={dispatch.busy} onClick={() => void dispatch.run('merge', item.ticketId!, item.boardId ?? undefined, onChanged)}>Erneut mergen</button>;
    }
    if (item.kind === 'note' && item.noteId !== null) {
      return <button type="button" className="todo-action" disabled={busy} onClick={() => void take()}>Ticket daraus</button>;
    }
    return <Link to={target.to} state={target.state} className="todo-action">{item.kind === 'question' ? 'Antworten' : 'Öffnen'}</Link>;
  };

  return (
    <li className={`todo-row${flag ? ' flagged' : ''}${urgent ? ' urgent' : ''}`} data-key={item.key}>
      {flag && <span className="todo-urgent" aria-label={urgent ? 'seit über 24 Std.' : undefined}>{urgent ? '!' : ''}</span>}
      <span className="mono todo-id">{id}</span>
      <div className="todo-main">
        {item.kind === 'note' && item.noteId !== null && item.boardId !== null
          ? <button type="button" className="todo-title" onClick={() => onOpenNote(item)}>{item.title}</button>
          : <Link to={target.to} state={target.state} className="todo-title">{item.title}</Link>}
        <span className="muted todo-detail">{detail}</span>
        {(error ?? dispatch.error) && <span className="field-error" role="alert">{error ?? dispatch.error}</span>}
        {dispatch.outcome && <span role="status" className="muted">{dispatch.outcome}</span>}
      </div>
      {action()}
    </li>
  );
}
