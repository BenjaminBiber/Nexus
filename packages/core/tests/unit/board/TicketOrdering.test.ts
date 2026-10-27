import { describe, expect, it } from 'vitest';
import { TicketOrdering } from '../../../src/board/TicketOrdering.js';
import type { Priority, TicketStatus } from '../../../src/board/BoardTypes.js';
import { ticketFixture } from './helpers/BoardDatabaseContract.js';

describe('TicketOrdering', () => {
  const ordering = new TicketOrdering();
  const t = (n: number, status: TicketStatus, priority: Priority = 'normal') =>
    ticketFixture({ id: `ARC-${String(n).padStart(4, '0')}`, projectId: 1, boardId: 1, number: n, status, priority });

  it('sortiert nach Spalte, Priorität und Nummer; Wartet steht nach dem Backlog', () => {
    const list = [t(5, 'done'), t(3, 'planned'), t(2, 'backlog', 'low'), t(4, 'backlog', 'urgent'), t(1, 'backlog'), t(6, 'backlog')];
    const sorted = ordering.sort(list, new Set(['ARC-0006']));
    expect(sorted.map((x) => x.number)).toEqual([4, 1, 2, 6, 3, 5]);
  });

  it('ordnet wartende Tickets der Spalte „waiting“ zu', () => {
    expect(ordering.column(t(1, 'planned'), true)).toBe('waiting');
    expect(ordering.column(t(1, 'planned'), false)).toBe('planned');
  });
});
