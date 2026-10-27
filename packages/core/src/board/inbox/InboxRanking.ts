import { MERGE_REASON_PREFIX, SETUP_REASON_PREFIX } from '../../dispatch/DispatchConfig.js';
import { isHumanInteraction, PRIORITY_RANK, type Priority, type Ticket } from '../BoardTypes.js';
import { INBOX_KINDS, type InboxItem, type InboxKind, type InboxTicketKind } from './InboxTypes.js';

// Art und Dringlichkeit der Einträge von „Meine Aufgaben“.
export class InboxRanking {
  // Art eines Tickets aus Status und Blockiert-Grund; null = wartet nicht auf den Benutzer
  classify(ticket: Pick<Ticket, 'status' | 'blockedReason'>): InboxTicketKind | null {
    if (ticket.status === 'in_planning') return 'planning';
    if (ticket.status !== 'blocked') return null;
    const reason = (ticket.blockedReason ?? '').trim();
    if (reason.startsWith(MERGE_REASON_PREFIX)) return 'merge';
    if (reason.startsWith(SETUP_REASON_PREFIX)) return 'setup';
    if (isHumanInteraction(reason)) return 'accept';
    return 'blocked';
  }

  // Rang einer Art (1 = am dringendsten)
  rank(kind: InboxKind): number {
    return INBOX_KINDS.indexOf(kind) + 1;
  }

  // Vergleich für die Standardsortierung: Rang, bei Fragen mehr wartende Tickets zuerst, bei Tickets höhere Priorität,
  // dann der ältere Eintrag, zuletzt der Schlüssel (stabil)
  compare(a: InboxItem, b: InboxItem): number {
    return a.rank - b.rank
      || b.waitingTickets - a.waitingTickets
      || this.priority(a) - this.priority(b)
      || a.since.localeCompare(b.since)
      || a.key.localeCompare(b.key);
  }

  // Liefert die Einträge nach Dringlichkeit sortiert (neue Liste)
  sort(items: InboxItem[]): InboxItem[] {
    return [...items].sort((a, b) => this.compare(a, b));
  }

  private priority(item: InboxItem): number {
    return item.priority ? PRIORITY_RANK[item.priority as Priority] ?? PRIORITY_RANK.normal : PRIORITY_RANK.normal;
  }
}
