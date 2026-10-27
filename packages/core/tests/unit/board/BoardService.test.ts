import { describe, expect, it } from 'vitest';
import type { TicketStatus } from '../../../src/board/BoardTypes.js';
import { ticketFixture, USER } from './helpers/BoardDatabaseContract.js';
import { BoardFixture } from './helpers/BoardFixture.js';

// Legt ein Ticket direkt in der DB an (TicketService gibt es erst in Task 7)
async function addTicket(f: BoardFixture, n: number, status: TicketStatus, dependsOn: string[] = [], boardId = f.boardId) {
  await f.db.transaction(async (u) => {
    const project = (await u.projects.byKey('ARC'))!;
    await u.tickets.insert(ticketFixture({ id: `ARC-${String(n).padStart(4, '0')}`, projectId: project.id, boardId, number: n, status, dependsOn }));
  });
}

describe('BoardService', () => {
  it('listet Boards mit Projekt, Besitzer und offenen Tickets', async () => {
    const f = await BoardFixture.create();
    await addTicket(f, 1, 'backlog');
    await addTicket(f, 2, 'done');
    const [summary] = await f.boards.list({ projectKey: 'arc' });
    expect(summary).toMatchObject({ id: f.boardId, name: 'Sprint', projectKey: 'ARC', ownerId: USER.id, ownerName: USER.name, openTickets: 1, repoPath: null });
    expect(await f.boards.list({ ownerId: '22222222-2222-2222-2222-222222222222' })).toEqual([]);
  });

  it('meldet ein unbekanntes Projekt beim Anlegen', async () => {
    const f = await BoardFixture.create();
    await expect(f.boards.create('XYZ', 'B', f.actor)).rejects.toMatchObject({ code: 'project-not-found' });
  });

  it('benennt um und setzt den Repo-Pfad', async () => {
    const f = await BoardFixture.create();
    expect((await f.boards.rename(f.boardId, 'Sprint 2')).name).toBe('Sprint 2');
    expect((await f.boards.setRepoPath(f.boardId, 'C:\\Projects\\Arches')).repoPath).toBe('C:\\Projects\\Arches');
    expect((await f.boards.setRepoPath(f.boardId, ' ')).repoPath).toBeNull();
    await expect(f.boards.rename(999, 'x')).rejects.toMatchObject({ code: 'board-not-found' });
  });

  it('schreibt den Repo-Pfad lokal statt in die gemeinsame DB', async () => {
    const f = await BoardFixture.create();
    const set = await f.boards.setRepoPath(f.boardId, 'C:\\Projects\\Arches');
    expect(set).toMatchObject({ repoPath: 'C:\\Projects\\Arches', repoSource: 'local' });
    expect((await f.db.transaction((u) => u.boards.byId(f.boardId)))?.repoPath).toBeNull();
    expect(await f.repoStore.read()).toEqual(new Map([[f.boardId, 'C:\\Projects\\Arches']]));
    expect((await f.boards.require(f.boardId)).repoPath).toBe('C:\\Projects\\Arches');
    expect((await f.boards.rename(f.boardId, 'Neu')).repoPath).toBe('C:\\Projects\\Arches');
    expect((await f.db.transaction((u) => u.boards.byId(f.boardId)))?.repoPath).toBeNull();
    await expect(f.boards.setRepoPath(999, 'C:/x')).rejects.toMatchObject({ code: 'board-not-found' });
  });

  it('nimmt den DB-Wert nur bei eigenen Boards als Altbestand, nie bei fremden', async () => {
    const f = await BoardFixture.create();
    const other = { id: '22222222-2222-2222-2222-222222222222', name: 'Kollege' };
    const projectId = (await f.db.transaction((u) => u.projects.byKey('ARC')))!.id;
    const foreign = await f.db.transaction(async (u) => {
      await u.users.upsert(other);
      const own = (await u.boards.byId(f.boardId))!;
      await u.boards.update({ ...own, repoPath: 'C:/alt' });
      return u.boards.insert({ projectId, ownerId: other.id, name: 'Fremd', repoPath: 'D:/kollege', createdAt: '2026-10-13T10:00:00.000Z' });
    });
    const list = await f.boards.list();
    expect(list.find((b) => b.id === f.boardId)).toMatchObject({ repoPath: 'C:/alt', repoSource: 'legacy' });
    expect(list.find((b) => b.id === foreign.id)).toMatchObject({ repoPath: null, repoSource: null });
    expect((await f.boards.require(foreign.id)).repoPath).toBeNull();
    expect(await f.boards.owned(USER.id)).toEqual([expect.objectContaining({ id: f.boardId, repoPath: 'C:/alt' })]);
    // Umbenennen lässt den DB-Wert stehen
    await f.boards.rename(f.boardId, 'Neu');
    expect((await f.db.transaction((u) => u.boards.byId(f.boardId)))?.repoPath).toBe('C:/alt');
    await f.boards.setRepoPath(f.boardId, null);
    expect((await f.boards.require(f.boardId)).repoPath).toBeNull();
  });

  it('verweigert das Löschen mit offenen Tickets und nennt sie', async () => {
    const f = await BoardFixture.create();
    await addTicket(f, 1, 'backlog');
    await expect(f.boards.delete(f.boardId, { deleteOpen: false }, f.actor)).rejects.toMatchObject({
      code: 'board-has-open-tickets', message: expect.stringContaining('ARC-0001'),
    });
  });

  it('löscht ein Board mit erledigten Tickets und protokolliert jede Löschung', async () => {
    const f = await BoardFixture.create();
    await addTicket(f, 1, 'done');
    await addTicket(f, 2, 'not_needed');
    const result = await f.boards.delete(f.boardId, { deleteOpen: false }, f.actor);
    expect(result.deletedTickets).toEqual(['ARC-0001', 'ARC-0002']);
    expect(await f.boards.list()).toEqual([]);
    expect((await f.db.transaction((u) => u.deletions.list())).map((d) => d.ticketId)).toEqual(['ARC-0001', 'ARC-0002']);
  });

  it('löscht mit deleteOpen alles und entfernt Verweise aus anderen Boards samt Verlauf', async () => {
    const f = await BoardFixture.create();
    const other = await f.boards.create('ARC', 'Anderes', f.actor);
    await addTicket(f, 1, 'backlog');
    await addTicket(f, 2, 'backlog', ['ARC-0001'], other.id);
    await f.boards.delete(f.boardId, { deleteOpen: true }, f.actor);
    const kept = await f.db.transaction((u) => u.tickets.byId('ARC-0002'));
    expect(kept?.dependsOn).toEqual([]);
    expect(kept?.rev).toBe(2);
    const history = await f.db.transaction((u) => u.history.list('ARC-0002'));
    expect(history).toMatchObject([{ field: 'dependsOn', from: '["ARC-0001"]', to: '[]', note: 'Voraussetzung gelöscht' }]);
  });

  it('lehnt zu lange Boardnamen ab', async () => {
    const f = await BoardFixture.create();
    await expect(f.boards.create('ARC', 'n'.repeat(201), f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
    await expect(f.boards.rename(f.boardId, 'n'.repeat(201))).rejects.toMatchObject({ code: 'value-too-long' });
  });
});
