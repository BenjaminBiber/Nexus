import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { BoardDatabase } from '../../../../src/board/BoardDatabase.js';
import type { Ticket } from '../../../../src/board/BoardTypes.js';

export const USER = { id: '11111111-1111-1111-1111-111111111111', name: 'Florian' };
const NOW = '2026-10-13T10:00:00.000Z';

// Vollständiges Ticket mit Standardwerten für Tests
export function ticketFixture(overrides: Partial<Ticket> & Pick<Ticket, 'id' | 'projectId' | 'boardId' | 'number'>): Ticket {
  return {
    title: 'Titel', description: '', status: 'backlog', priority: 'normal', labels: [], dependsOn: [],
    sortOrder: 0, assignee: null, needsPlanning: true, acceptance: '', plan: '', handbook: '', diagram: '',
    spec: null, specTask: null, branch: null, worktree: null, targetBranch: null, touches: [], consumes: [], produces: [],
    blockedReason: null, plannedAt: null, plannedBy: null, planningClaimBy: null, planningClaimAt: null,
    createdAt: NOW, createdById: USER.id, createdByName: USER.name, updatedAt: NOW, rev: 1,
    ...overrides,
  };
}

// Gemeinsame Tests für jede Umsetzung von BoardDatabase (In-Memory und SQL Server)
export function boardDatabaseContract(name: string, create: () => Promise<BoardDatabase>, reset: () => Promise<void> = async () => {}) {
  describe(`BoardDatabase-Vertrag: ${name}`, () => {
    let db: BoardDatabase;

    beforeEach(async () => {
      await reset();
      db = await create();
      await db.transaction((u) => u.users.upsert(USER));
    });
    afterEach(() => db.close());

    // Legt Projekt und Board an und liefert beide IDs
    async function seed() {
      return db.transaction(async (u) => {
        const project = await u.projects.insert({ key: 'ARC', name: 'Arches', createdAt: NOW, createdBy: USER.id });
        const board = await u.boards.insert({ projectId: project.id, ownerId: USER.id, name: 'Sprint', repoPath: null, createdAt: NOW });
        return { projectId: project.id, boardId: board.id };
      });
    }

    it('aktualisiert den Namen eines Benutzers', async () => {
      await db.transaction((u) => u.users.upsert({ ...USER, name: 'Flo' }));
      expect(await db.transaction((u) => u.users.byId(USER.id))).toEqual({ ...USER, name: 'Flo' });
    });

    it('vergibt Ticketnummern pro Projekt fortlaufend', async () => {
      const { projectId } = await seed();
      const numbers = await db.transaction(async (u) => [await u.projects.takeTicketNumber(projectId), await u.projects.takeTicketNumber(projectId)]);
      expect(numbers).toEqual([1, 2]);
      expect((await db.transaction((u) => u.projects.byKey('ARC')))?.nextTicketNumber).toBe(3);
    });

    it('speichert Tickets mit Labels und Abhängigkeiten in ihrer Reihenfolge', async () => {
      const { projectId, boardId } = await seed();
      await db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0002', projectId, boardId, number: 2 }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0003', projectId, boardId, number: 3, labels: ['ui', 'api'], dependsOn: ['ARC-0002', 'ARC-0001'], title: 'Umlaute äöü' }));
      });
      const t = await db.transaction((u) => u.tickets.byId('ARC-0003'));
      expect(t).toMatchObject({ labels: ['ui', 'api'], dependsOn: ['ARC-0002', 'ARC-0001'], title: 'Umlaute äöü', rev: 1 });
      expect(await db.transaction((u) => u.tickets.dependents('ARC-0001'))).toEqual(['ARC-0003']);
      expect((await db.transaction((u) => u.tickets.list({ boardIds: [boardId] }))).map((x) => x.id).sort()).toEqual(['ARC-0001', 'ARC-0002', 'ARC-0003']);
      expect((await db.transaction((u) => u.tickets.list({ ids: ['ARC-0002'] }))).map((x) => x.id)).toEqual(['ARC-0002']);
    });

    it('lehnt ein Update mit veralteter Version als conflict ab', async () => {
      const { projectId, boardId } = await seed();
      await db.transaction((u) => u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 })));
      await db.transaction(async (u) => {
        const t = (await u.tickets.byId('ARC-0001', true))!;
        await u.tickets.update({ ...t, title: 'Neu', rev: 2 });
      });
      await expect(db.transaction(async (u) => {
        const t = (await u.tickets.byId('ARC-0001'))!;
        await u.tickets.update({ ...t, title: 'Zu spät', rev: 2 });
      })).rejects.toMatchObject({ code: 'conflict' });
      expect((await db.transaction((u) => u.tickets.byId('ARC-0001')))?.title).toBe('Neu');
    });

    it('rollt bei einem Fehler die ganze Transaktion zurück', async () => {
      const { projectId } = await seed();
      await expect(db.transaction(async (u) => {
        await u.projects.takeTicketNumber(projectId);
        throw new Error('Abbruch');
      })).rejects.toThrow('Abbruch');
      expect((await db.transaction((u) => u.projects.byId(projectId)))?.nextTicketNumber).toBe(1);
    });

    it('entfernt Verweise auf ein Ticket und löscht es', async () => {
      const { projectId, boardId } = await seed();
      await db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0002', projectId, boardId, number: 2, dependsOn: ['ARC-0001'] }));
      });
      const touched = await db.transaction(async (u) => {
        const ids = await u.tickets.removeDependencyOn('ARC-0001');
        await u.tickets.delete('ARC-0001');
        return ids;
      });
      expect(touched).toEqual(['ARC-0002']);
      expect(await db.transaction((u) => u.tickets.byId('ARC-0001'))).toBeNull();
      expect((await db.transaction((u) => u.tickets.byId('ARC-0002')))?.dependsOn).toEqual([]);
    });

    it('liefert die höchste SortOrder einer Spalte', async () => {
      const { projectId, boardId } = await seed();
      expect(await db.transaction((u) => u.tickets.maxSortOrder(boardId, 'backlog'))).toBeNull();
      await db.transaction((u) => u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1, sortOrder: 4 })));
      expect(await db.transaction((u) => u.tickets.maxSortOrder(boardId, 'backlog'))).toBe(4);
    });

    it('zählt offene Tickets je Board ohne erledigte und nicht mehr nötige', async () => {
      const { projectId, boardId } = await seed();
      await db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0002', projectId, boardId, number: 2, status: 'in_progress' }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0003', projectId, boardId, number: 3, status: 'done' }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0004', projectId, boardId, number: 4, status: 'not_needed' }));
      });
      expect(await db.transaction((u) => u.tickets.openCountByBoard([boardId, boardId + 999]))).toEqual({ [boardId]: 2 });
      expect(await db.transaction((u) => u.tickets.openCountByBoard([]))).toEqual({});
    });

    it('liefert ohne Texte dieselben Tickets mit Labels und Voraussetzungen', async () => {
      const { projectId, boardId } = await seed();
      await db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1, description: 'lang' }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0002', projectId, boardId, number: 2, labels: ['a', 'b'], dependsOn: ['ARC-0001'] }));
      });
      const light = await db.transaction((u) => u.tickets.list({ withoutTexts: true }));
      expect(light.map((t) => [t.id, t.status, t.labels, t.dependsOn])).toEqual([
        ['ARC-0001', 'backlog', [], []],
        ['ARC-0002', 'backlog', ['a', 'b'], ['ARC-0001']],
      ]);
      // Vorhanden bleibt erkennbar, leer bleibt leer
      expect(light.map((t) => [t.description.trim() !== '', t.plan])).toEqual([[true, ''], [false, '']]);
    });

    it('speichert Kommentare, Verlauf, Notizen und Löschungen', async () => {
      const { projectId, boardId } = await seed();
      await db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 }));
        await u.comments.insert({ ticketId: 'ARC-0001', authorId: USER.id, authorName: 'claude', body: 'Hallo', createdAt: NOW });
        await u.history.insert([{ ticketId: 'ARC-0001', at: NOW, byId: USER.id, byName: 'claude', field: 'status', from: null, to: 'backlog', note: 'angelegt' }]);
        const note = await u.notes.insert({ boardId, authorId: USER.id, authorName: 'claude', title: 'Cache leert sich', body: 'Befund', source: 'ARC-0001', labels: ['cache'], seenCount: 1, seenIn: [], state: 'open', ticketId: null, reason: null, createdAt: NOW });
        await u.notes.update({ ...note, seenCount: 3, seenIn: ['ARC-0002', 'ARC-0003'], state: 'dropped', reason: 'egal' });
        await u.deletions.insert({ at: NOW, byId: USER.id, byName: 'claude', ticketId: 'ARC-0009', ticketTitle: 'Weg', fromStatus: 'backlog' });
      });
      const comments = await db.transaction((u) => u.comments.list('ARC-0001'));
      expect(comments).toMatchObject([{ body: 'Hallo', authorName: 'claude', createdAt: NOW, readByAgentAt: null }]);
      expect(await db.transaction((u) => u.history.list('ARC-0001'))).toMatchObject([{ field: 'status', to: 'backlog', note: 'angelegt' }]);
      const notes = await db.transaction((u) => u.notes.list({ boardId, states: ['dropped'] }));
      expect(notes).toMatchObject([{ title: 'Cache leert sich', body: 'Befund', labels: ['cache'], seenCount: 3, seenIn: ['ARC-0002', 'ARC-0003'], reason: 'egal', source: 'ARC-0001' }]);
      expect(await db.transaction((u) => u.notes.list({ boardId, states: ['open'] }))).toEqual([]);
      expect(await db.transaction((u) => u.deletions.list())).toMatchObject([{ ticketId: 'ARC-0009' }]);
    });

    it('löscht ein Board samt Notizen', async () => {
      const { boardId } = await seed();
      await db.transaction((u) => u.notes.insert({ boardId, authorId: USER.id, authorName: 'x', title: '', body: 'n', source: null, labels: [], seenCount: 1, seenIn: [], state: 'open', ticketId: null, reason: null, createdAt: NOW }));
      await db.transaction((u) => u.boards.delete(boardId));
      expect(await db.transaction((u) => u.boards.byId(boardId))).toBeNull();
    });

    it('ändert den Änderungskennwert bei Tickets, Kommentaren, Notizen und Löschungen, nicht bei anderen Boards', async () => {
      const { projectId, boardId } = await seed();
      const other = await db.transaction((u) => u.boards.insert({ projectId, ownerId: USER.id, name: 'Anderes', repoPath: null, createdAt: NOW }));
      const otherBefore = await db.changeMarker(other.id);
      const m0 = await db.changeMarker(boardId);
      await db.transaction((u) => u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 })));
      const m1 = await db.changeMarker(boardId);
      expect(m1).not.toBe(m0);
      await db.transaction((u) => u.comments.insert({ ticketId: 'ARC-0001', authorId: USER.id, authorName: 'F', body: 'x', createdAt: NOW }));
      const m2 = await db.changeMarker(boardId);
      expect(m2).not.toBe(m1);
      await db.transaction((u) => u.notes.insert({
        boardId, authorId: USER.id, authorName: 'F', title: '', body: 'n', source: null, labels: [], seenCount: 1, seenIn: [], state: 'open', ticketId: null, reason: null, createdAt: NOW,
      }));
      const m3 = await db.changeMarker(boardId);
      expect(m3).not.toBe(m2);
      await db.transaction(async (u) => {
        await u.tickets.delete('ARC-0001');
        await u.deletions.insert({ at: NOW, byId: USER.id, byName: 'F', ticketId: 'ARC-0001', ticketTitle: 'Titel', fromStatus: 'backlog' });
      });
      const m4 = await db.changeMarker(boardId);
      expect(m4).not.toBe(m3);
      expect(await db.changeMarker(boardId)).toBe(m4);
      expect(await db.changeMarker(other.id)).toBe(otherBefore);
    });

    it('lässt den Kennwert bei einer zurückgerollten Transaktion unverändert', async () => {
      const { projectId, boardId } = await seed();
      const before = await db.changeMarker(boardId);
      await expect(db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 }));
        throw new Error('Abbruch');
      })).rejects.toThrow('Abbruch');
      expect(await db.changeMarker(boardId)).toBe(before);
    });

    it('zählt die Kommentare je Ticket eines Boards', async () => {
      const { projectId, boardId } = await seed();
      await db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0002', projectId, boardId, number: 2 }));
        for (const body of ['a', 'b']) await u.comments.insert({ ticketId: 'ARC-0001', authorId: USER.id, authorName: 'F', body, createdAt: NOW });
      });
      expect(await db.transaction((u) => u.comments.countByBoard(boardId))).toEqual({ 'ARC-0001': 2 });
    });

    it('speichert Spec, Aufgabe, Branch, Worktree, Zielbranch und die Listen am Ticket', async () => {
      const { projectId, boardId } = await seed();
      await db.transaction((u) => u.tickets.insert(ticketFixture({
        id: 'ARC-0001', projectId, boardId, number: 1,
        spec: '001-login', specTask: 'T001', targetBranch: 'release/1', touches: ['src/login', 'src/äöü'], consumes: ['GET /a'], produces: ['<Login>'],
      })));
      expect(await db.transaction((u) => u.tickets.byId('ARC-0001'))).toMatchObject({
        spec: '001-login', specTask: 'T001', branch: null, worktree: null, targetBranch: 'release/1',
        touches: ['src/login', 'src/äöü'], consumes: ['GET /a'], produces: ['<Login>'],
      });
      await db.transaction(async (u) => {
        const t = (await u.tickets.byId('ARC-0001', true))!;
        await u.tickets.update({ ...t, branch: 'ticket/ARC-0001-login', worktree: 'C:/x/ARC-0001', targetBranch: null, touches: [], rev: 2 });
      });
      expect(await db.transaction((u) => u.tickets.byId('ARC-0001'))).toMatchObject({ branch: 'ticket/ARC-0001-login', worktree: 'C:/x/ARC-0001', targetBranch: null, touches: [] });
    });

    it('speichert Schritte in Reihenfolge, hakt ab, ersetzt sie und zählt den Fortschritt', async () => {
      const { projectId, boardId } = await seed();
      await db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0002', projectId, boardId, number: 2 }));
        await u.steps.replace('ARC-0001', [{ text: 'Test schreiben', done: false, detail: '```ts\nexpect(1).toBe(1);\n```' }, { text: 'Umsetzen', done: false }]);
      });
      expect(await db.transaction((u) => u.steps.list('ARC-0001'))).toEqual([
        { text: 'Test schreiben', done: false, detail: '```ts\nexpect(1).toBe(1);\n```' },
        { text: 'Umsetzen', done: false },
      ]);
      expect(await db.transaction((u) => u.steps.setDone('ARC-0001', 1, true))).toBe(true);
      expect(await db.transaction((u) => u.steps.setDone('ARC-0001', 5, true))).toBe(false);
      expect(await db.transaction((u) => u.steps.progress(['ARC-0001', 'ARC-0002']))).toEqual({ 'ARC-0001': { done: 1, total: 2 } });
      await db.transaction((u) => u.steps.replace('ARC-0001', [{ text: 'Nur noch einer', done: true }]));
      expect(await db.transaction((u) => u.steps.list('ARC-0001'))).toEqual([{ text: 'Nur noch einer', done: true }]);
      expect(await db.transaction((u) => u.steps.progress([]))).toEqual({});
    });

    it('verknüpft Sessions genau einmal, findet die Tickets einer Session und ändert den Kennwert', async () => {
      const { projectId, boardId } = await seed();
      await db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0002', projectId, boardId, number: 2 }));
      });
      const before = await db.changeMarker(boardId);
      expect(await db.transaction((u) => u.ticketSessions.add('ARC-0002', 's-1', NOW))).toBe(true);
      expect(await db.changeMarker(boardId)).not.toBe(before);
      expect(await db.transaction((u) => u.ticketSessions.add('ARC-0002', 's-1', NOW))).toBe(false);
      await db.transaction(async (u) => {
        await u.ticketSessions.add('ARC-0001', 's-1', NOW);
        await u.ticketSessions.add('ARC-0001', 's-2', '2026-10-13T11:00:00.000Z');
      });
      expect(await db.transaction((u) => u.ticketSessions.ticketsOf('s-1'))).toEqual(['ARC-0001', 'ARC-0002']);
      expect(await db.transaction((u) => u.ticketSessions.ticketsOf('s-9'))).toEqual([]);
      expect(await db.transaction((u) => u.ticketSessions.byTickets(['ARC-0001', 'ARC-0002']))).toEqual({ 'ARC-0001': ['s-1', 's-2'], 'ARC-0002': ['s-1'] });
    });

    it('liefert Kommentare ohne Lesemarke und setzt sie', async () => {
      const { projectId, boardId } = await seed();
      const ids = await db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 }));
        const a = await u.comments.insert({ ticketId: 'ARC-0001', authorId: USER.id, authorName: 'Florian', body: 'Bitte prüfen', createdAt: NOW });
        await u.comments.insert({ ticketId: 'ARC-0001', authorId: USER.id, authorName: 'claude', body: 'Eigener', createdAt: NOW, readByAgentAt: NOW });
        const c = await u.comments.insert({ ticketId: 'ARC-0001', authorId: USER.id, authorName: 'Florian', body: 'Noch was', createdAt: NOW });
        return [a.id, c.id];
      });
      expect((await db.transaction((u) => u.comments.unreadByAgent(['ARC-0001']))).map((c) => c.body)).toEqual(['Bitte prüfen', 'Noch was']);
      await db.transaction((u) => u.comments.markReadByAgent([ids[0]], '2026-10-13T12:00:00.000Z'));
      expect((await db.transaction((u) => u.comments.unreadByAgent(['ARC-0001']))).map((c) => c.body)).toEqual(['Noch was']);
      expect((await db.transaction((u) => u.comments.list('ARC-0001')))[0].readByAgentAt).toBe('2026-10-13T12:00:00.000Z');
      expect(await db.transaction((u) => u.comments.unreadByAgent([]))).toEqual([]);
    });

    it('löscht Schritte und Sessions mit dem Ticket', async () => {
      const { projectId, boardId } = await seed();
      await db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId, boardId, number: 1 }));
        await u.steps.replace('ARC-0001', [{ text: 'A', done: false }]);
        await u.ticketSessions.add('ARC-0001', 's-1', NOW);
      });
      await db.transaction((u) => u.tickets.delete('ARC-0001'));
      expect(await db.transaction((u) => u.steps.list('ARC-0001'))).toEqual([]);
      expect(await db.transaction((u) => u.ticketSessions.ticketsOf('s-1'))).toEqual([]);
    });
  });
}
