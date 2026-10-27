import type { BoardHistoryEntry } from '@harness/core';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempSqlite } from './helpers/TempSqlite.js';

const USER = '11111111-1111-1111-1111-111111111111';
const NOW = '2026-10-13T10:00:00.000Z';

function entry(over: Partial<BoardHistoryEntry> = {}): BoardHistoryEntry {
  return { ticketId: 'ARC-1', at: NOW, byId: USER, byName: 'Tester', field: 'status', from: 'backlog', to: 'planned', note: null, ...over };
}

describe('SQLite: Verlauf', () => {
  let temp: TempSqlite;

  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  it('schreibt und liest in Reihenfolge des Schreibens', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });

    const e1 = entry({ at: '2026-10-13T10:00:00.000Z' });
    const e2 = entry({ at: '2026-10-13T10:05:00.000Z', field: 'title', from: 'Alt', to: 'Neu äöü', note: 'Grund' });
    await temp.db.transaction((u) => u.history.insert([e1, e2]));
    const e3 = entry({ at: '2026-10-13T10:10:00.000Z', from: null, to: null });
    await temp.db.transaction((u) => u.history.insert([e3]));

    expect(await temp.db.transaction((u) => u.history.list('ARC-1'))).toEqual([e1, e2, e3]);
  });

  it('liefert nur Einträge des Tickets', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });
    await temp.seedTicket({ id: 'ARC-2', projectId, boardId, number: 2 });

    const e1 = entry();
    const e2 = entry({ ticketId: 'ARC-2' });
    await temp.db.transaction((u) => u.history.insert([e1, e2]));

    expect(await temp.db.transaction((u) => u.history.list('ARC-1'))).toEqual([e1]);
    expect(await temp.db.transaction((u) => u.history.list('ARC-2'))).toEqual([e2]);
  });

  it('normiert GUID und Datum', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });

    await temp.db.transaction((u) => u.history.insert([entry({ byId: USER.toUpperCase(), at: '2026-10-13T10:00:00Z' })]));

    const [read] = await temp.db.transaction((u) => u.history.list('ARC-1'));
    expect(read.byId).toBe(USER);
    expect(read.at).toBe('2026-10-13T10:00:00.000Z');
  });

  it('leere Liste schreibt nichts', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });

    await expect(temp.db.transaction((u) => u.history.insert([]))).resolves.toBeUndefined();
    expect(await temp.db.transaction((u) => u.history.list('ARC-1'))).toEqual([]);
  });
});
