import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempSqlite, TEST_NOW, TEST_USER } from './helpers/TempSqlite.js';

describe('SqliteCommentRepository', () => {
  let temp: TempSqlite;
  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  const nc = (ticketId: string, body: string, readByAgentAt?: string | null) => ({
    ticketId, authorId: TEST_USER.id, authorName: 'Test', body, createdAt: TEST_NOW,
    ...(readByAgentAt === undefined ? {} : { readByAgentAt }),
  });

  it('legt einen Kommentar an und liefert seine Id', async () => {
    const { boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId: 1, boardId, number: 1 });

    const c1 = await temp.db.transaction((u) => u.comments.insert(nc('ARC-1', 'Erster')));
    expect(typeof c1.id).toBe('number');
    expect(c1.readByAgentAt).toBeNull();

    const c2 = await temp.db.transaction((u) => u.comments.insert(nc('ARC-1', 'Zweiter')));
    expect(c2.id).toBeGreaterThan(c1.id);
  });

  it('listet Kommentare eines Tickets in Reihenfolge des Anlegens', async () => {
    const { boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId: 1, boardId, number: 1 });
    await temp.seedTicket({ id: 'ARC-2', projectId: 1, boardId, number: 2 });

    const c1 = await temp.db.transaction((u) => u.comments.insert(nc('ARC-1', 'Erster')));
    const c2 = await temp.db.transaction((u) => u.comments.insert(nc('ARC-1', 'Zweiter')));

    await expect(temp.db.transaction((u) => u.comments.list('ARC-1'))).resolves.toEqual([c1, c2]);
    await expect(temp.db.transaction((u) => u.comments.list('ARC-2'))).resolves.toEqual([]);
  });

  it('zählt Kommentare je Ticket eines Boards', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    const other = (await temp.seedBoard(projectId, 'Andere')).boardId;
    await temp.seedTicket({ id: 'ARC-1', projectId, boardId, number: 1 });
    await temp.seedTicket({ id: 'ARC-2', projectId, boardId, number: 2 });
    await temp.seedTicket({ id: 'ARC-4', projectId, boardId: other, number: 4 });

    await temp.db.transaction((u) => u.comments.insert(nc('ARC-1', 'a')));
    await temp.db.transaction((u) => u.comments.insert(nc('ARC-1', 'b')));
    await temp.db.transaction((u) => u.comments.insert(nc('ARC-2', 'x')));
    await temp.db.transaction((u) => u.comments.insert(nc('ARC-4', 'y')));

    await expect(temp.db.transaction((u) => u.comments.countByBoard(boardId))).resolves.toEqual({ 'ARC-1': 2, 'ARC-2': 1 });
  });

  it('setzt und liest die Lesemarke des Agenten', async () => {
    const { boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId: 1, boardId, number: 1 });
    await temp.seedTicket({ id: 'ARC-2', projectId: 1, boardId, number: 2 });

    const c1 = await temp.db.transaction((u) => u.comments.insert(nc('ARC-1', 'Erster')));
    const c2 = await temp.db.transaction((u) => u.comments.insert(nc('ARC-1', 'Zweiter')));
    const c3 = await temp.db.transaction((u) => u.comments.insert(nc('ARC-2', 'gelesen', '2026-10-13T11:00:00.000Z')));
    expect(c3.readByAgentAt).toBe('2026-10-13T11:00:00.000Z');

    await expect(temp.db.transaction((u) => u.comments.unreadByAgent(['ARC-1', 'ARC-2']))).resolves.toEqual([c1, c2]);

    await temp.db.transaction((u) => u.comments.markReadByAgent([c1.id], '2026-10-13T12:00:00.000Z'));
    const afterFirst = await temp.db.transaction((u) => u.comments.list('ARC-1'));
    expect(afterFirst[0].readByAgentAt).toBe('2026-10-13T12:00:00.000Z');

    await temp.db.transaction((u) => u.comments.markReadByAgent([c1.id, c2.id], '2026-10-13T13:00:00.000Z'));
    const afterSecond = await temp.db.transaction((u) => u.comments.list('ARC-1'));
    expect(afterSecond[0].readByAgentAt).toBe('2026-10-13T12:00:00.000Z');
    expect(afterSecond[1].readByAgentAt).toBe('2026-10-13T13:00:00.000Z');

    await expect(temp.db.transaction((u) => u.comments.unreadByAgent(['ARC-1']))).resolves.toEqual([]);
    await expect(temp.db.transaction((u) => u.comments.unreadByAgent([]))).resolves.toEqual([]);
    await expect(temp.db.transaction((u) => u.comments.markReadByAgent([], TEST_NOW))).resolves.not.toThrow();
  });

  it('löscht Kommentare per Kaskade mit ihrem Ticket', async () => {
    const { boardId } = await temp.seedBoard();
    await temp.seedTicket({ id: 'ARC-1', projectId: 1, boardId, number: 1 });
    await temp.db.transaction((u) => u.comments.insert(nc('ARC-1', 'Erster')));

    await temp.sql("DELETE FROM Tickets WHERE Id = 'ARC-1'");

    await expect(temp.db.transaction((u) => u.comments.list('ARC-1'))).resolves.toEqual([]);
    const { rows } = await temp.sql<{ N: number }>("SELECT COUNT(*) AS N FROM Comments WHERE TicketId = 'ARC-1'");
    expect(Number(rows[0].N)).toBe(0);
  });
});
