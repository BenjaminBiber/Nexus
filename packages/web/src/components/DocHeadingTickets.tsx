import { Link } from 'react-router-dom';
import { DB_COLUMN_LABELS } from '../api/dbTypes.js';
import type { DocTicketRefDto } from '../api/types.js';

// Ein Chip je Ticket: die ID als Text, Titel und Status im Hinweis, Klick führt auf das Ticket im Board.
// Der Status steckt zusätzlich im aria-label, damit er auch ohne die Farbe lesbar bleibt.
export function DocTicketChip({ pid, ticket }: { pid: string; ticket: DocTicketRefDto }) {
  const status = DB_COLUMN_LABELS[ticket.column];
  return (
    <Link
      className="doc-ticket-chip"
      data-status={ticket.column}
      to={`/p/${pid}/board/t/${ticket.id}`}
      title={`${ticket.title} · ${status}`}
      aria-label={`${ticket.id}: ${ticket.title}, ${status}`}
    >
      {ticket.id}
    </Link>
  );
}

// Die Chips der Tickets, die aus einer Überschrift entstanden sind; sie liegen frei im Layout an der
// Stelle der Überschrift, damit der gerenderte Markdown unangetastet bleibt.
export function DocHeadingTickets({ pid, headingId, tickets, top, left }: {
  pid: string;
  headingId: string;
  tickets: DocTicketRefDto[];
  top: number;
  left: number;
}) {
  return (
    <span className="doc-heading-tickets" data-testid={`heading-tickets-${headingId}`} style={{ top, left }}>
      {tickets.map((t) => (
        <DocTicketChip key={t.id} pid={pid} ticket={t} />
      ))}
    </span>
  );
}
