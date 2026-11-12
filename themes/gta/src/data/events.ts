// Ereignisse für Banner: Ticket erledigt (Mission Passed), Ticket blockiert (Busted), Kontingent leer (Wasted).
// Reine Funktionen über zwei aufeinanderfolgende Stände.
import { TICKET_REWARD } from './derive.js';
import type { BoardView, ColumnId } from './types.js';

export type GameEvent =
  | { kind: 'passed'; ticketId: string; title: string; reward: number }
  | { kind: 'busted'; ticketId: string; title: string; reason: string | null }
  | { kind: 'wasted' };

function columnsById(view: BoardView): Map<string, ColumnId> {
  return new Map(view.columns.flatMap((column) => column.tickets.map((card) => [card.id, card.column] as const)));
}

/** Vergleich zweier Board-Stände. Ohne Vorgänger oder bei anderem Board: nichts. Nur Tickets, die es vorher schon gab:
    Spalte wird done → passed (reward = TICKET_REWARD); Spalte wird blocked → busted mit Blockiert-Grund. */
export function boardEvents(prev: BoardView | null, next: BoardView): GameEvent[] {
  if (!prev || prev.boardId !== next.boardId) return [];
  const before = columnsById(prev);
  const events: GameEvent[] = [];
  for (const card of next.columns.flatMap((column) => column.tickets)) {
    const was = before.get(card.id);
    if (was === undefined || was === card.column) continue;
    if (card.column === 'done') events.push({ kind: 'passed', ticketId: card.id, title: card.title, reward: TICKET_REWARD });
    if (card.column === 'blocked') events.push({ kind: 'busted', ticketId: card.id, title: card.title, reason: card.blockedReason });
  }
  return events;
}

/** 5-Stunden-Kontingent erreicht 100 %: vorher bekannt und unter 100, jetzt ab 100 → wasted; sonst null */
export function quotaEvent(prevPercent: number | null, nextPercent: number | null): GameEvent | null {
  return prevPercent !== null && nextPercent !== null && prevPercent < 100 && nextPercent >= 100 ? { kind: 'wasted' } : null;
}
