import { describe, expect, it } from 'vitest';
import { SpecProgress } from '../../../src/board/SpecProgress.js';
import type { Ticket, TicketStatus } from '../../../src/board/BoardTypes.js';

// Minimales Ticket mit Spec
function ticket(id: string, spec: string | null, status: TicketStatus, deps: string[] = []): Ticket {
  return {
    id, projectId: 1, boardId: 1, number: 1, title: id, description: '', status, priority: 'normal', labels: [],
    dependsOn: deps, sortOrder: 0, assignee: null, needsPlanning: false, acceptance: '', plan: '', handbook: '', diagram: '',
    spec, specTask: null, branch: null, worktree: null, targetBranch: null, touches: [], consumes: [], produces: [], blockedReason: null,
    plannedAt: null, plannedBy: null, planningClaimBy: null, planningClaimAt: null, createdAt: '', createdById: '',
    createdByName: '', updatedAt: '', rev: 1,
  };
}

describe('SpecProgress.count', () => {
  it('ordnet jedes Ticket genau einer Gruppe zu; nicht mehr nötig zählt nicht zur Summe', () => {
    const tickets = [
      ticket('A-1', '002-b', 'done'),
      ticket('A-2', '001-a', 'done'),
      ticket('A-3', '001-a', 'blocked'),
      ticket('A-4', '001-a', 'planned', ['A-5']),
      ticket('A-5', '001-a', 'backlog'),
      ticket('A-6', '001-a', 'in_progress', ['A-5']),
      ticket('A-7', '001-a', 'not_needed'),
      ticket('A-8', null, 'planned'),
    ];
    expect(new SpecProgress().count(tickets)).toEqual([
      { spec: '001-a', total: 5, done: 1, blocked: 1, waiting: 1, inProgress: 1, open: 1, notNeeded: 1 },
      { spec: '002-b', total: 1, done: 1, blocked: 0, waiting: 0, inProgress: 0, open: 0, notNeeded: 0 },
    ]);
  });

  it('löst Voraussetzungen über alle Tickets auf, auch außerhalb der gezählten', () => {
    const counted = [ticket('A-1', '001-a', 'planned', ['B-1'])];
    const all = [...counted, ticket('B-1', null, 'in_progress')];
    expect(new SpecProgress().count(counted, all)[0]).toMatchObject({ waiting: 1, open: 0 });
  });
});
