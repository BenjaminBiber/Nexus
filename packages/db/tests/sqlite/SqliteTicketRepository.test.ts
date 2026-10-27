import type { Ticket } from '@harness/core';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { ticketFixture } from '../../../core/tests/unit/board/helpers/BoardDatabaseContract.js';
import { TempSqlite } from './helpers/TempSqlite.js';

describe('SqliteTicketRepository', () => {
  let temp: TempSqlite;
  let projectId: number;
  let boardId: number;
  let other: number;

  beforeEach(async () => {
    temp = await TempSqlite.create();
    ({ projectId, boardId } = await temp.seedBoard());
    other = (await temp.seedBoard(projectId, 'Andere')).boardId;
  });
  afterEach(async () => { await temp.dispose(); });

  // Ticket mit den Pflichtfeldern des aktuellen Boards
  const fx = (id: string, number: number, o: Partial<Ticket> = {}): Ticket => ticketFixture({ id, projectId, boardId, number, ...o });

  it('legt ein Ticket an und liest es mit allen Feldern', async () => {
    const t1 = fx('ARC-1', 1, {
      labels: ['zwei', 'eins'], touches: ['packages/db'], consumes: ['A'], produces: ['B'],
      spec: '001', specTask: 'T001', branch: 'ticket/ARC-1', worktree: 'C:/wt', assignee: 'claude',
      needsPlanning: false, plannedAt: '2026-10-14T08:30:00.000Z', plannedBy: 'claude',
      blockedReason: 'Grund', plan: '# Plan', handbook: 'H', diagram: 'flowchart LR',
    });
    await temp.db.transaction((u) => u.tickets.insert(t1));

    expect(await temp.db.transaction((u) => u.tickets.byId('ARC-1'))).toEqual(t1);
    // forUpdate ist ein No-op und liefert dasselbe Ticket
    expect(await temp.db.transaction((u) => u.tickets.byId('ARC-1', true))).toEqual(t1);
    expect(await temp.db.transaction((u) => u.tickets.byId('ARC-9'))).toBeNull();
  });

  it('behält die Reihenfolge der Voraussetzungen', async () => {
    await temp.db.transaction(async (u) => {
      await u.tickets.insert(fx('ARC-1', 1));
      await u.tickets.insert(fx('ARC-2', 2));
      await u.tickets.insert(fx('ARC-3', 3, { dependsOn: ['ARC-2', 'ARC-1'] }));
    });

    expect((await temp.db.transaction((u) => u.tickets.byId('ARC-3')))?.dependsOn).toEqual(['ARC-2', 'ARC-1']);
    expect(await temp.db.transaction((u) => u.tickets.dependents('ARC-1'))).toEqual(['ARC-3']);
  });

  it('filtert und sortiert in list', async () => {
    await temp.db.transaction(async (u) => {
      await u.tickets.insert(fx('ARC-1', 1, { labels: ['a'] }));
      await u.tickets.insert(fx('ARC-2', 2, { boardId: other }));
      await u.tickets.insert(fx('ARC-3', 3, { labels: ['b'] }));
    });
    const list = (filter?: { boardIds?: number[]; ids?: string[] }) => temp.db.transaction((u) => u.tickets.list(filter));
    const ids = async (filter?: { boardIds?: number[]; ids?: string[] }) => (await list(filter)).map((t) => t.id);

    expect(await ids()).toEqual(['ARC-1', 'ARC-2', 'ARC-3']);
    expect(await ids({ boardIds: [boardId] })).toEqual(['ARC-1', 'ARC-3']);
    expect((await list({ boardIds: [boardId] })).map((t) => t.labels)).toEqual([['a'], ['b']]);
    expect(await ids({ ids: ['ARC-3', 'ARC-1'] })).toEqual(['ARC-1', 'ARC-3']);
    expect(await list({ boardIds: [] })).toEqual([]);
    expect(await list({ ids: [] })).toEqual([]);
  });

  it('ändert ein Ticket und lehnt eine veraltete Version als conflict ab', async () => {
    const t = fx('ARC-1', 1, { labels: ['a'] });
    await temp.db.transaction((u) => u.tickets.insert(t));

    await temp.db.transaction((u) => u.tickets.update({ ...t, title: 'Neu', labels: ['x'], rev: 2 }));
    expect(await temp.db.transaction((u) => u.tickets.byId('ARC-1'))).toMatchObject({ title: 'Neu', labels: ['x'], rev: 2 });

    await expect(temp.db.transaction((u) => u.tickets.update({ ...t, title: 'Alt', rev: 2 }))).rejects.toMatchObject({ code: 'conflict' });
    expect((await temp.db.transaction((u) => u.tickets.byId('ARC-1')))?.title).toBe('Neu');
  });

  it('verschiebt ein Ticket in ein anderes Board', async () => {
    const t = fx('ARC-1', 1);
    await temp.db.transaction((u) => u.tickets.insert(t));

    await temp.db.transaction((u) => u.tickets.update({ ...t, boardId: other, status: 'in_progress', sortOrder: 5, rev: 2 }));

    expect(await temp.db.transaction((u) => u.tickets.list({ boardIds: [other] }))).toMatchObject([{ id: 'ARC-1', status: 'in_progress' }]);
    expect(await temp.db.transaction((u) => u.tickets.maxSortOrder(other, 'in_progress'))).toBe(5);
    expect(await temp.db.transaction((u) => u.tickets.maxSortOrder(boardId, 'backlog'))).toBeNull();
  });

  it('entfernt Verweise auf ein Ticket und liefert deren IDs', async () => {
    await temp.db.transaction(async (u) => {
      await u.tickets.insert(fx('ARC-1', 1));
      await u.tickets.insert(fx('ARC-2', 2, { dependsOn: ['ARC-1'] }));
      await u.tickets.insert(fx('ARC-3', 3, { dependsOn: ['ARC-1'] }));
    });

    expect(await temp.db.transaction((u) => u.tickets.removeDependencyOn('ARC-1'))).toEqual(['ARC-2', 'ARC-3']);
    expect(await temp.db.transaction((u) => u.tickets.dependents('ARC-1'))).toEqual([]);
    expect((await temp.db.transaction((u) => u.tickets.byId('ARC-2')))?.dependsOn).toEqual([]);
  });

  it('löscht ein Ticket samt Verweisen und Kindzeilen', async () => {
    await temp.db.transaction(async (u) => {
      await u.tickets.insert(fx('ARC-1', 1, { labels: ['a'] }));
      await u.tickets.insert(fx('ARC-2', 2, { dependsOn: ['ARC-1'] }));
    });

    await temp.db.transaction((u) => u.tickets.delete('ARC-1'));

    expect(await temp.db.transaction((u) => u.tickets.byId('ARC-1'))).toBeNull();
    expect((await temp.db.transaction((u) => u.tickets.byId('ARC-2')))?.dependsOn).toEqual([]);
    const { rows } = await temp.sql<{ N: number }>("SELECT COUNT(*) AS N FROM TicketLabels WHERE TicketId = 'ARC-1'");
    expect(Number(rows[0].N)).toBe(0);
  });
});
