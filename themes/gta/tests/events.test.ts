// Ereignisse aus zwei Board-Ständen und aus dem 5-Stunden-Kontingent
import { describe, expect, it } from 'vitest';
import { TICKET_REWARD } from '../src/data/derive.js';
import { boardEvents, quotaEvent } from '../src/data/events.js';
import type { BoardView, Card, ColumnId } from '../src/data/types.js';

const card = (id: string, column: ColumnId, blockedReason: string | null = null): Card => ({
  id,
  title: `Titel ${id}`,
  column,
  blockedReason,
  stepsDone: 0,
  stepsTotal: 0,
  priority: 'normal',
  assignee: null,
});

// Board aus Karten; die Spalten ergeben sich aus den Karten
function board(boardId: number, cards: Card[]): BoardView {
  const ids = [...new Set(cards.map((c) => c.column))];
  return { boardId, columns: ids.map((id) => ({ id, title: id, tickets: cards.filter((c) => c.column === id) })) };
}

const BEFORE = board(1, [card('N-1', 'in_progress'), card('N-2', 'in_progress'), card('N-3', 'planned'), card('N-4', 'blocked', 'alt')]);

describe('boardEvents', () => {
  it('ohne Vorgänger oder bei anderem Board: nichts', () => {
    const next = board(1, [card('N-1', 'done')]);
    expect(boardEvents(null, next)).toEqual([]);
    expect(boardEvents(board(2, [card('N-1', 'in_progress')]), next)).toEqual([]);
  });

  it('erledigt → passed, blockiert → busted mit Grund', () => {
    const next = board(1, [card('N-1', 'done'), card('N-2', 'blocked', 'Zugang fehlt'), card('N-3', 'blocked'), card('N-4', 'blocked', 'alt')]);
    expect(boardEvents(BEFORE, next)).toEqual([
      { kind: 'passed', ticketId: 'N-1', title: 'Titel N-1', reward: TICKET_REWARD },
      { kind: 'busted', ticketId: 'N-2', title: 'Titel N-2', reason: 'Zugang fehlt' },
      { kind: 'busted', ticketId: 'N-3', title: 'Titel N-3', reason: null },
    ]);
  });

  it('neue Tickets, unveränderte und andere Wechsel lösen nichts aus', () => {
    const next = board(1, [card('N-1', 'in_progress'), card('N-2', 'planned'), card('N-3', 'planned'), card('N-4', 'in_progress'), card('N-9', 'done'), card('N-8', 'blocked')]);
    expect(boardEvents(BEFORE, next)).toEqual([]);
  });

  it('erledigt bleibt erledigt: kein zweites passed', () => {
    const done = board(1, [card('N-1', 'done')]);
    expect(boardEvents(done, done)).toEqual([]);
  });
});

describe('quotaEvent', () => {
  it('wasted, wenn das Kontingent 100 % erreicht', () => {
    expect(quotaEvent(99, 100)).toEqual({ kind: 'wasted' });
    expect(quotaEvent(0, 120)).toEqual({ kind: 'wasted' });
  });

  it('sonst null', () => {
    expect(quotaEvent(null, 100)).toBeNull();
    expect(quotaEvent(100, 100)).toBeNull();
    expect(quotaEvent(101, 100)).toBeNull();
    expect(quotaEvent(50, 99)).toBeNull();
    expect(quotaEvent(50, null)).toBeNull();
  });
});
