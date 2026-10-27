import { useState, type DragEvent } from 'react';
import { Link } from 'react-router-dom';
import { useApi } from '../../api/ApiContext.js';
import { INBOX_KIND_LABELS, type InboxItemDto } from '../../api/inboxTypes.js';
import { formatDate } from '../../dbboard/formatDate.js';
import { useDispatchAction } from '../../dbboard/useDispatchAction.js';

// Ziel eines Eintrags: Ticket-Panel, Frage oder Notizen-Dialog des Boards (wie bei der Suche)
export function inboxTarget(pid: string, item: InboxItemDto): { to: string; state?: unknown } {
  const p = encodeURIComponent(pid);
  if (item.kind === 'question' && item.questionId) return { to: `/p/${p}/questions/${encodeURIComponent(item.questionId)}` };
  if (item.ticketId) return { to: `/p/${p}/board/t/${encodeURIComponent(item.ticketId)}` };
  return { to: `/p/${p}/board`, state: item.boardId ? { notesBoard: item.boardId } : undefined };
}

export interface InboxRowProps {
  pid: string;
  item: InboxItemDto;
  index: number;
  // false: Umsortieren aus (z. B. mit Filter)
  draggable: boolean;
  onChanged: () => void;
  onDragStart: (index: number) => void;
  onDrop: (index: number) => void;
}

// Eine Zeile: Art, Titel mit Link, Board, Detail, „seit …“ und Aktion der Art; umsortiert wird per Ziehen und Ablegen.
export function InboxRow({ pid, item, index, draggable, onChanged, onDragStart, onDrop }: InboxRowProps) {
  const api = useApi();
  const dispatch = useDispatchAction();
  const [takeError, setTakeError] = useState<string | null>(null);
  const [taking, setTaking] = useState(false);
  // Hier würde ein gezogener Eintrag abgelegt
  const [over, setOver] = useState(false);
  const target = inboxTarget(pid, item);
  const id = item.questionId ?? item.ticketId ?? (item.noteId !== null ? `Notiz ${item.noteId}` : '');

  const take = async () => {
    if (item.noteId === null) return;
    setTaking(true);
    setTakeError(null);
    try {
      await api.takeDbNote(item.noteId);
      onChanged();
    } catch (e) {
      setTakeError((e as Error).message);
    } finally {
      setTaking(false);
    }
  };

  const action = () => {
    if (item.kind === 'accept' && item.ticketId) {
      return <button type="button" className="btn small primary" disabled={dispatch.busy} onClick={() => void dispatch.run('accept', item.ticketId!, item.boardId ?? undefined, onChanged)}>Abnehmen</button>;
    }
    if (item.kind === 'merge' && item.ticketId) {
      return <button type="button" className="btn small primary" disabled={dispatch.busy} onClick={() => void dispatch.run('merge', item.ticketId!, item.boardId ?? undefined, onChanged)}>Erneut mergen</button>;
    }
    if (item.kind === 'note') {
      return <button type="button" className="btn small" disabled={taking} onClick={() => void take()}>Ticket daraus</button>;
    }
    return null;
  };

  const dragProps = draggable
    ? {
        draggable: true,
        onDragStart: (event: DragEvent<HTMLLIElement>) => {
          event.dataTransfer?.setData('text/plain', item.key);
          onDragStart(index);
        },
        onDragOver: (event: DragEvent<HTMLLIElement>) => {
          event.preventDefault();
          setOver(true);
        },
        onDragLeave: () => setOver(false),
        onDrop: (event: DragEvent<HTMLLIElement>) => {
          event.preventDefault();
          setOver(false);
          onDrop(index);
        },
      }
    : {};
  const error = dispatch.error ?? takeError;

  return (
    <li className={`inbox-row kind-${item.kind}${over ? ' drop-target' : ''}`} data-key={item.key} {...dragProps}>
      <div className="inbox-main">
        <span className="badge inbox-badge">{INBOX_KIND_LABELS[item.kind]}</span>
        <Link to={target.to} state={target.state} className="inbox-title">
          {id && <span className="mono inbox-id">{id}</span>} {item.title}
        </Link>
        <span className="muted inbox-meta">
          {item.projectKey}
          {item.boardName ? ` · ${item.boardName}` : ''}
          {item.kind === 'question' ? ` · ${item.waitingTickets} ${item.waitingTickets === 1 ? 'Ticket wartet' : 'Tickets warten'}` : ''}
          {' · seit '}
          {formatDate(item.since)}
        </span>
        {item.detail && <p className="inbox-detail">{item.detail}</p>}
        {error && <p className="field-error" role="alert">{error}</p>}
        {dispatch.outcome && <p role="status" className="muted">{dispatch.outcome}</p>}
      </div>
      <div className="inbox-actions">
        {action()}
      </div>
    </li>
  );
}
