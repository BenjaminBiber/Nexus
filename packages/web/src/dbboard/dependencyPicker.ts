import type { DbColumnId, DbStatus } from '../api/dbTypes.js';

// Auswahl der Voraussetzungen im Ticket-Panel (aus deps-picker.js des alten Boards). Die Rechnung sieht nur die
// Tickets des Boards; unbekannte IDs, Selbstbezug und Kreise prüft beim Speichern ohnehin der Server.

/** Spalten, in denen ein Ticket nichts mehr voraussetzt: nur lesen, nicht anbieten */
export const CLOSED_STATUSES: DbStatus[] = ['done', 'not_needed'];

/** Zusatz an einem Eintrag, der einen Kreis ergäbe */
export const CYCLE_REASON = 'setzt dieses Ticket voraus';

export interface PickerTicket {
  id: string;
  title: string;
  boardId: number;
  status: DbStatus;
  column: DbColumnId;
  dependsOn: string[];
}

export interface PickerEntry {
  id: string;
  title: string;
  disabled: boolean;
  reason: string | null;
}

export interface PickerGroup {
  id: DbColumnId;
  title: string;
  entries: PickerEntry[];
}

// Lassen sich die Voraussetzungen dieses Tickets bearbeiten?
export function canEditDependencies(ticket: { status: DbStatus }): boolean {
  return !CLOSED_STATUSES.includes(ticket.status);
}

// IDs aller Tickets, die `id` direkt oder über Umwege voraussetzen (ein gespeicherter Kreis hält die Suche nicht auf)
export function dependentsClosure(id: string, tickets: PickerTicket[]): Set<string> {
  const found = new Set<string>();
  const queue = [id];
  while (queue.length) {
    const current = queue.pop()!;
    for (const ticket of tickets) {
      if (ticket.id === id || found.has(ticket.id) || !ticket.dependsOn.includes(current)) continue;
      found.add(ticket.id);
      queue.push(ticket.id);
    }
  }
  return found;
}

// Menüinhalt: offene Tickets desselben Boards ohne das Ticket selbst und ohne schon eingetragene,
// gruppiert nach Anzeige-Spalte (leere Gruppen fallen weg); Kreis-Kandidaten bleiben sichtbar, aber gesperrt
export function pickerGroups(ticket: PickerTicket, tickets: PickerTicket[], columns: Array<{ id: DbColumnId; title: string }>): PickerGroup[] {
  const board = tickets.filter((candidate) => candidate.boardId === ticket.boardId);
  const listed = new Set(ticket.dependsOn);
  const dependents = dependentsClosure(ticket.id, board);
  const offered = board.filter((c) => c.id !== ticket.id && !listed.has(c.id) && !CLOSED_STATUSES.includes(c.status));
  return columns
    .map((column) => ({
      id: column.id,
      title: column.title,
      entries: offered
        .filter((c) => c.column === column.id)
        .map((c) => {
          const disabled = dependents.has(c.id);
          return { id: c.id, title: c.title, disabled, reason: disabled ? CYCLE_REASON : null };
        }),
    }))
    .filter((group) => group.entries.length > 0);
}

// Liste plus `id` hinten dran, ohne Doppelte
export function withDependency(list: string[], id: string): string[] {
  return list.includes(id) ? [...list] : [...list, id];
}

// Liste ohne `id`
export function withoutDependency(list: string[], id: string): string[] {
  return list.filter((entry) => entry !== id);
}
