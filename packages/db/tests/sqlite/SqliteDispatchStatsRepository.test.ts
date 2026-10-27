import { InMemoryBoardDatabase, type BoardDatabase, type BoardHistoryEntry, type BoardUnitOfWork } from '@harness/core';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { ticketFixture, USER } from '../../../core/tests/unit/board/helpers/BoardDatabaseContract.js';
import { TempSqlite } from './helpers/TempSqlite.js';

const NOW = '2026-10-13T10:00:00.000Z';

// Statuswechsel mit Standardwerten (byId USER, byName 'x', from null, note null)
function change(ticketId: string, field: string, at: string, to: string): BoardHistoryEntry {
  return { ticketId, at, byId: USER.id, byName: 'x', field, from: null, to, note: null };
}

interface Seeded { arc: number; oth: number; b1: number; b2: number; b3: number }

// Legt Projekte ARC (Boards „Eins“, „Zwei“), OTH (Board „Drei“), die vier Tickets, den Verlauf und die Kommentare an
async function seed(uow: BoardUnitOfWork): Promise<Seeded> {
  await uow.users.upsert(USER);
  const arc = await uow.projects.insert({ key: 'ARC', name: 'Arches', createdAt: NOW, createdBy: USER.id });
  const oth = await uow.projects.insert({ key: 'OTH', name: 'Andere', createdAt: NOW, createdBy: USER.id });
  const b1 = await uow.boards.insert({ projectId: arc.id, ownerId: USER.id, name: 'Eins', repoPath: null, createdAt: NOW });
  const b2 = await uow.boards.insert({ projectId: arc.id, ownerId: USER.id, name: 'Zwei', repoPath: null, createdAt: NOW });
  const b3 = await uow.boards.insert({ projectId: oth.id, ownerId: USER.id, name: 'Drei', repoPath: null, createdAt: NOW });

  await uow.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId: arc.id, boardId: b1.id, number: 1, status: 'done' }));
  await uow.tickets.insert(ticketFixture({ id: 'ARC-0002', projectId: arc.id, boardId: b2.id, number: 2, status: 'done' }));
  await uow.tickets.insert(ticketFixture({ id: 'ARC-0003', projectId: arc.id, boardId: b1.id, number: 3, status: 'done' }));
  await uow.tickets.insert(ticketFixture({ id: 'OTH-0001', projectId: oth.id, boardId: b3.id, number: 1, status: 'done' }));

  await uow.history.insert([
    change('ARC-0001', 'status', '2026-10-02T10:00:00.000Z', 'done'),
    change('ARC-0001', 'title', '2026-10-02T11:00:00.000Z', 'x'),
    change('ARC-0001', 'blockedReason', '2026-10-02T12:00:00.000Z', 'warte'),
    change('ARC-0002', 'status', '2026-10-03T10:00:00.000Z', 'done'),
    change('ARC-0003', 'status', '2026-10-03T10:00:00.000Z', 'done'),
    change('ARC-0003', 'status', '2026-10-04T10:00:00.000Z', 'in_progress'),
    change('ARC-0003', 'status', '2026-10-20T10:00:00.000Z', 'done'),
    change('OTH-0001', 'status', '2026-10-03T10:00:00.000Z', 'done'),
  ]);

  for (const body of [
    '[dispatch] Start: an junior-ARC-0001 übergeben',
    '[dispatch] Abbruch Versuch 1/3: x.',
    '[dispatch] Start: wieder',
    'Mensch: [dispatch] Abbruch',
    '[DISPATCH] Start groß',
  ]) {
    await uow.comments.insert({ ticketId: 'ARC-0001', authorId: USER.id, authorName: 'dispatcher', body, createdAt: NOW });
  }
  await uow.comments.insert({ ticketId: 'OTH-0001', authorId: USER.id, authorName: 'dispatcher', body: '[dispatch] Start: an junior-OTH-0001', createdAt: NOW });

  return { arc: arc.id, oth: oth.id, b1: b1.id, b2: b2.id, b3: b3.id };
}

describe('SQLite: Auswertung der Dispatcher-Läufe', () => {
  let temp: TempSqlite;
  let mem: BoardDatabase;
  let sqliteSeed: Seeded;
  let memSeed: Seeded;

  beforeEach(async () => {
    temp = await TempSqlite.create();
    mem = new InMemoryBoardDatabase();
    sqliteSeed = await temp.db.transaction((u) => seed(u));
    memSeed = await mem.transaction((u) => seed(u));
  });
  afterEach(async () => {
    await temp.dispose();
    await mem.close();
  });

  it('findet Tickets mit letztem Wechsel nach Erledigt im Zeitraum (mit und ohne Board-Filter)', async () => {
    const range = { projectId: sqliteSeed.arc, from: '2026-10-01T00:00:00.000Z', to: '2026-10-10T00:00:00.000Z' };
    const memRange = { ...range, projectId: memSeed.arc };

    const result = await temp.db.transaction((u) => u.dispatchStats.doneTicketIds(range));
    expect(result).toEqual(['ARC-0001', 'ARC-0002']);
    expect(result).toEqual(await mem.transaction((u) => u.dispatchStats.doneTicketIds(memRange)));

    const withBoard = await temp.db.transaction((u) => u.dispatchStats.doneTicketIds({ ...range, boardId: sqliteSeed.b2 }));
    expect(withBoard).toEqual(['ARC-0002']);
    expect(withBoard).toEqual(await mem.transaction((u) => u.dispatchStats.doneTicketIds({ ...memRange, boardId: memSeed.b2 })));

    const withoutMs = await temp.db.transaction((u) => u.dispatchStats.doneTicketIds({ ...range, from: '2026-10-01T00:00:00Z' }));
    expect(withoutMs).toEqual(['ARC-0001', 'ARC-0002']);
  });

  it('liefert die Statuswechsel und Blockiert-Gründe der angefragten Tickets in Reihenfolge des Eintrags', async () => {
    const history = await temp.db.transaction((u) => u.dispatchStats.statusHistory(['ARC-0001', 'ARC-0003']));
    expect(history.map((h) => [h.ticketId, h.field, h.to])).toEqual([
      ['ARC-0001', 'status', 'done'],
      ['ARC-0001', 'blockedReason', 'warte'],
      ['ARC-0003', 'status', 'done'],
      ['ARC-0003', 'status', 'in_progress'],
      ['ARC-0003', 'status', 'done'],
    ]);
    expect(history).toEqual(await mem.transaction((u) => u.dispatchStats.statusHistory(['ARC-0001', 'ARC-0003'])));

    expect(await temp.db.transaction((u) => u.dispatchStats.statusHistory([]))).toEqual([]);
  });

  it('zählt [dispatch]-Start- und Abbruch-Kommentare je Ticket (groß geschrieben zählt nicht)', async () => {
    const counts = await temp.db.transaction((u) => u.dispatchStats.dispatchCommentCounts(['ARC-0001', 'ARC-0002']));
    expect(counts).toEqual({ 'ARC-0001': { starts: 2, aborts: 1 } });
    expect(counts).toEqual(await mem.transaction((u) => u.dispatchStats.dispatchCommentCounts(['ARC-0001', 'ARC-0002'])));

    expect(await temp.db.transaction((u) => u.dispatchStats.dispatchCommentCounts([]))).toEqual({});
  });

  it('liefert die [dispatch]-Kommentare des Projekts in Anlagereihenfolge', async () => {
    const comments = await temp.db.transaction((u) => u.dispatchStats.dispatchComments(sqliteSeed.arc));
    expect(comments.map((c) => c.body)).toEqual([
      '[dispatch] Start: an junior-ARC-0001 übergeben',
      '[dispatch] Abbruch Versuch 1/3: x.',
      '[dispatch] Start: wieder',
    ]);

    const memComments = await mem.transaction((u) => u.dispatchStats.dispatchComments(memSeed.arc));
    expect(comments.map(({ id: _id, ...rest }) => rest)).toEqual(memComments.map(({ id: _id, ...rest }) => rest));
  });
});
