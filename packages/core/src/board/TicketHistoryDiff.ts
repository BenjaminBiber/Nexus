import type { BoardActor, HistoryEntry, Ticket } from './BoardTypes.js';

// Felder in der Reihenfolge, in der sie im Verlauf erscheinen
const SCALARS = ['title', 'status', 'priority', 'assignee', 'needsPlanning', 'acceptance', 'blockedReason', 'boardId', 'plannedBy', 'planningClaimBy', 'spec', 'specTask', 'branch', 'worktree', 'targetBranch'] as const;
const LISTS = ['labels', 'dependsOn', 'touches', 'consumes', 'produces'] as const;
const LONG_TEXT = ['description', 'plan', 'handbook', 'diagram'] as const;

// Erzeugt Verlaufseinträge für alle geänderten Felder eines Tickets.
export class TicketHistoryDiff {
  // Vergleicht Vorher und Nachher; SortOrder, Rev und Zeitstempel werden nie protokolliert
  diff(before: Ticket, after: Ticket, actor: BoardActor, at: string, note: string | null): HistoryEntry[] {
    const entries: HistoryEntry[] = [];
    const add = (field: string, from: string | null, to: string | null) =>
      entries.push({ ticketId: after.id, at, byId: actor.userId, byName: actor.name, field, from, to, note });
    for (const field of SCALARS) {
      if (before[field] !== after[field]) add(field, this.scalar(before[field]), this.scalar(after[field]));
    }
    for (const field of LISTS) {
      const from = JSON.stringify(before[field]);
      const to = JSON.stringify(after[field]);
      if (from !== to) add(field, from, to);
    }
    for (const field of LONG_TEXT) {
      if (before[field] !== after[field]) add(field, `${before[field].length} Zeichen`, `${after[field].length} Zeichen`);
    }
    return entries;
  }

  // Einzelwert als Text; null bleibt null
  private scalar(value: string | number | boolean | null): string | null {
    return value === null ? null : String(value);
  }
}
