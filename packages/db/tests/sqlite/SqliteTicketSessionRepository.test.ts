import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempSqlite, TEST_NOW } from './helpers/TempSqlite.js';

const T9 = '2026-10-13T09:00:00.000Z';
const T10 = TEST_NOW;
const T11 = '2026-10-13T11:00:00.000Z';

describe('SqliteTicketSessionRepository', () => {
  let temp: TempSqlite;
  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  it('erzeugt bei doppeltem Eintragen derselben Session keine zweite Zeile', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });

    const first = await temp.db.transaction((u) => u.ticketSessions.add('ARC-1', 's1', T10));
    expect(first).toBe(true);
    const second = await temp.db.transaction((u) => u.ticketSessions.add('ARC-1', 's1', T11));
    expect(second).toBe(false);

    const { rows: countRows } = await temp.sql<{ N: number }>("SELECT COUNT(*) AS N FROM TicketSessions WHERE TicketId = 'ARC-1' AND SessionId = 's1'");
    expect(Number(countRows[0].N)).toBe(1);
    const { rows: atRows } = await temp.sql<{ AddedAt: string }>('SELECT AddedAt FROM TicketSessions');
    expect(atRows[0].AddedAt).toBe(T10);
  });

  it('liefert byTickets in der Reihenfolge der Verknüpfung', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });
    await temp.seedTicket({ id: 'ARC-2', projectId, boardId, number: 2 });
    await temp.seedTicket({ id: 'ARC-3', projectId, boardId, number: 3 });

    await temp.db.transaction((u) => u.ticketSessions.add('ARC-1', 's1', T10));
    await temp.db.transaction((u) => u.ticketSessions.add('ARC-1', 's2', T9));
    await temp.db.transaction((u) => u.ticketSessions.add('ARC-2', 's1', T11));

    const result = await temp.db.transaction((u) => u.ticketSessions.byTickets(['ARC-1', 'ARC-2', 'ARC-3']));
    expect(result).toEqual({ 'ARC-1': ['s2', 's1'], 'ARC-2': ['s1'] });

    const empty = await temp.db.transaction((u) => u.ticketSessions.byTickets([]));
    expect(empty).toEqual({});
  });

  it('liefert ticketsOf sortiert', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });
    await temp.seedTicket({ id: 'ARC-2', projectId, boardId, number: 2 });
    await temp.seedTicket({ id: 'ARC-3', projectId, boardId, number: 3 });

    await temp.db.transaction((u) => u.ticketSessions.add('ARC-1', 's1', T10));
    await temp.db.transaction((u) => u.ticketSessions.add('ARC-1', 's2', T9));
    await temp.db.transaction((u) => u.ticketSessions.add('ARC-2', 's1', T11));

    const sessions = await temp.db.transaction((u) => u.ticketSessions.ticketsOf('s1'));
    expect(sessions).toEqual(['ARC-1', 'ARC-2']);

    const none = await temp.db.transaction((u) => u.ticketSessions.ticketsOf('unbekannt'));
    expect(none).toEqual([]);
  });

  it('ändert den Änderungskennwert', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-3', projectId, boardId, number: 3 });

    const m0 = await temp.db.changeMarker(boardId);
    await temp.db.transaction((u) => u.ticketSessions.add('ARC-3', 's9', T10));
    const m1 = await temp.db.changeMarker(boardId);
    expect(m1).not.toBe(m0);
  });

  it('lässt Verknüpfungen mit dem Ticket entfallen', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });
    await temp.seedTicket({ id: 'ARC-2', projectId, boardId, number: 2 });

    await temp.db.transaction((u) => u.ticketSessions.add('ARC-1', 's1', T10));
    await temp.db.transaction((u) => u.ticketSessions.add('ARC-2', 's1', T11));

    await temp.sql("DELETE FROM Tickets WHERE Id = 'ARC-1'");
    const sessions = await temp.db.transaction((u) => u.ticketSessions.ticketsOf('s1'));
    expect(sessions).toEqual(['ARC-2']);
  });
});
