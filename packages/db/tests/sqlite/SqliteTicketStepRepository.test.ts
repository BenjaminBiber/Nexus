import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempSqlite } from './helpers/TempSqlite.js';

describe('SqliteTicketStepRepository', () => {
  let temp: TempSqlite;

  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  const a = { text: 'a', done: false };
  const b = { text: 'b', done: true };
  const c = { text: 'c', done: false, detail: 'npm test' };
  const x = { text: 'x', done: false };

  const positions = async () =>
    (await temp.sql<{ Position: number }>("SELECT Position FROM TicketSteps WHERE TicketId = 'ARC-1' ORDER BY Position")).rows.map((r) => Number(r.Position));
  const texts = async () => (await temp.db.transaction((u) => u.steps.list('ARC-1'))).map((s) => s.text);

  it('legt Schritte an', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });

    await temp.db.transaction((u) => u.steps.replace('ARC-1', [a, b, c]));

    expect(await temp.db.transaction((u) => u.steps.list('ARC-1'))).toEqual([a, b, c]);
    expect(await positions()).toEqual([0, 1, 2]);
  });

  it('fügt an einer Position ein', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });
    await temp.db.transaction((u) => u.steps.replace('ARC-1', [a, b, c]));

    await temp.db.transaction((u) => u.steps.replace('ARC-1', [a, x, b, c]));

    expect(await texts()).toEqual(['a', 'x', 'b', 'c']);
    expect(await positions()).toEqual([0, 1, 2, 3]);
    expect((await temp.db.transaction((u) => u.steps.list('ARC-1')))[2].done).toBe(true);
  });

  it('verschiebt Schritte', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });
    await temp.db.transaction((u) => u.steps.replace('ARC-1', [a, x, b, c]));

    await temp.db.transaction((u) => u.steps.replace('ARC-1', [c, a, x, b]));

    expect(await texts()).toEqual(['c', 'a', 'x', 'b']);
    expect(await positions()).toEqual([0, 1, 2, 3]);
  });

  it('löscht Schritte, Nummern bleiben lückenlos', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });
    await temp.db.transaction((u) => u.steps.replace('ARC-1', [c, a, x, b]));

    await temp.db.transaction((u) => u.steps.replace('ARC-1', [c, b]));
    expect(await texts()).toEqual(['c', 'b']);
    expect(await positions()).toEqual([0, 1]);

    await temp.db.transaction((u) => u.steps.replace('ARC-1', []));
    expect(await temp.db.transaction((u) => u.steps.list('ARC-1'))).toEqual([]);
    expect(await positions()).toEqual([]);
  });

  it('setzt den Haken eines Schritts', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });
    await temp.db.transaction((u) => u.steps.replace('ARC-1', [a, b]));

    expect(await temp.db.transaction((u) => u.steps.setDone('ARC-1', 0, true))).toBe(true);
    expect((await temp.db.transaction((u) => u.steps.list('ARC-1')))[0].done).toBe(true);

    expect(await temp.db.transaction((u) => u.steps.setDone('ARC-1', 0, false))).toBe(true);
    expect((await temp.db.transaction((u) => u.steps.list('ARC-1')))[0].done).toBe(false);

    expect(await temp.db.transaction((u) => u.steps.setDone('ARC-1', 9, true))).toBe(false);
  });

  it('liefert den Fortschritt je Ticket', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });
    await temp.seedTicket({ id: 'ARC-2', projectId, boardId, number: 2 });
    await temp.seedTicket({ id: 'ARC-3', projectId, boardId, number: 3 });
    await temp.db.transaction((u) => u.steps.replace('ARC-1', [b, a]));
    await temp.db.transaction((u) => u.steps.replace('ARC-2', [b]));

    expect(await temp.db.transaction((u) => u.steps.progress(['ARC-1', 'ARC-2', 'ARC-3']))).toEqual({
      'ARC-1': { done: 1, total: 2 },
      'ARC-2': { done: 1, total: 1 },
    });
    expect(await temp.db.transaction((u) => u.steps.progress([]))).toEqual({});
  });

  it('Schritte fallen mit dem Ticket', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });
    await temp.db.transaction((u) => u.steps.replace('ARC-1', [a]));

    await temp.sql("DELETE FROM Tickets WHERE Id = 'ARC-1'");

    expect(Number((await temp.sql<{ N: number }>('SELECT COUNT(*) AS N FROM TicketSteps')).rows[0].N)).toBe(0);
  });
});
