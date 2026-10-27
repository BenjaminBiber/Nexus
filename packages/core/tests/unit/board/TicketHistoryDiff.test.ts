import { describe, expect, it } from 'vitest';
import { TicketHistoryDiff } from '../../../src/board/TicketHistoryDiff.js';
import { ticketFixture, USER } from './helpers/BoardDatabaseContract.js';

const AT = '2026-10-13T11:00:00.000Z';
const actor = { userId: USER.id, name: 'claude' };

describe('TicketHistoryDiff', () => {
  const diff = new TicketHistoryDiff();
  const before = ticketFixture({ id: 'ARC-0001', projectId: 1, boardId: 1, number: 1 });

  it('schreibt nur geänderte Felder', () => {
    expect(diff.diff(before, { ...before, sortOrder: 9, rev: 2 }, actor, AT, null)).toEqual([]);
  });

  it('schreibt Status als Einzelwert mit Vermerk', () => {
    expect(diff.diff(before, { ...before, status: 'planned' }, actor, AT, 'verschoben')).toEqual([
      { ticketId: 'ARC-0001', at: AT, byId: USER.id, byName: 'claude', field: 'status', from: 'backlog', to: 'planned', note: 'verschoben' },
    ]);
  });

  it('schreibt lange Texte nur mit Zeichenzahl', () => {
    const [entry] = diff.diff(before, { ...before, plan: 'x'.repeat(120) }, actor, AT, null);
    expect(entry).toMatchObject({ field: 'plan', from: '0 Zeichen', to: '120 Zeichen' });
  });

  it('schreibt Listen als JSON', () => {
    const [entry] = diff.diff(before, { ...before, labels: ['ui'] }, actor, AT, null);
    expect(entry).toMatchObject({ field: 'labels', from: '[]', to: '["ui"]' });
  });

  it('schreibt Wahrheitswerte und leere Werte lesbar', () => {
    const entries = diff.diff(before, { ...before, needsPlanning: false, assignee: 'claude' }, actor, AT, null);
    expect(entries).toEqual([
      expect.objectContaining({ field: 'assignee', from: null, to: 'claude' }),
      expect.objectContaining({ field: 'needsPlanning', from: 'true', to: 'false' }),
    ]);
  });

  it('schreibt den Planungs-Claim als Einzelwert', () => {
    const [entry] = diff.diff(before, { ...before, planningClaimBy: 'junior-x', planningClaimAt: AT }, actor, AT, null);
    expect(entry).toMatchObject({ field: 'planningClaimBy', from: null, to: 'junior-x' });
  });
});
