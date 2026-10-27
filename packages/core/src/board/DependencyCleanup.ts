import type { BoardUnitOfWork } from './BoardDatabase.js';
import type { BoardActor } from './BoardTypes.js';
import { TicketHistoryDiff } from './TicketHistoryDiff.js';

// Entfernt Verweise anderer Tickets auf ein gelöschtes Ticket, erhöht deren Version und schreibt den Verlauf.
export class DependencyCleanup {
  private readonly diff = new TicketHistoryDiff();

  // skip: Tickets, die in derselben Aktion ohnehin gelöscht werden (kein Verlauf nötig)
  async remove(u: BoardUnitOfWork, removedId: string, actor: BoardActor, at: string, skip: Set<string> = new Set()): Promise<string[]> {
    const touched = await u.tickets.removeDependencyOn(removedId);
    for (const ticketId of touched) {
      if (skip.has(ticketId)) continue;
      // removeDependencyOn hat den Verweis schon entfernt; „before“ stellt den alten Stand für den Verlauf her
      const current = (await u.tickets.byId(ticketId, true))!;
      const before = { ...current, dependsOn: [...current.dependsOn, removedId] };
      const after = { ...current, rev: current.rev + 1, updatedAt: at };
      await u.tickets.update(after);
      await u.history.insert(this.diff.diff(before, after, actor, at, 'Voraussetzung gelöscht'));
    }
    return touched;
  }
}
