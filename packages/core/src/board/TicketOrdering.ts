import { PRIORITY_RANK, type Ticket, type TicketStatus } from './BoardTypes.js';

export type BoardColumn = TicketStatus | 'waiting';
export const COLUMN_ORDER: BoardColumn[] = ['backlog', 'waiting', 'in_planning', 'planned', 'in_progress', 'blocked', 'done', 'not_needed'];

// Reihenfolge der Anzeige: Spalte, dann Priorität (dringend zuerst), dann Ticketnummer.
export class TicketOrdering {
  // Spalte eines Tickets; wartende Tickets stehen in „waiting“
  column(ticket: Ticket, waiting: boolean): BoardColumn {
    return waiting ? 'waiting' : ticket.status;
  }

  // Liefert eine sortierte Kopie
  sort(tickets: Ticket[], waitingIds: Set<string>): Ticket[] {
    const col = (t: Ticket) => COLUMN_ORDER.indexOf(this.column(t, waitingIds.has(t.id)));
    return [...tickets].sort((a, b) =>
      col(a) - col(b) || PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority] || a.number - b.number || a.id.localeCompare(b.id));
  }
}
