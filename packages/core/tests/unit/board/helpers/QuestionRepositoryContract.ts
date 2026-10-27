import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { BoardDatabase, BoardQuestionFilter } from '../../../../src/board/BoardDatabase.js';
import type { BoardQuestion } from '../../../../src/board/BoardQuestionTypes.js';
import { USER, ticketFixture } from './BoardDatabaseContract.js';

const NOW = '2026-10-14T10:00:00.000Z';
const LATER = '2026-10-14T11:00:00.000Z';

// Vollständige Frage mit Standardwerten für Tests
export function questionFixture(overrides: Partial<BoardQuestion> & Pick<BoardQuestion, 'id' | 'projectId' | 'number'>): BoardQuestion {
  return {
    title: 'Welcher Speicher?', description: '', state: 'open', occasion: null, originKind: 'session', originName: 'claude', originSession: null,
    tags: [], ticketIds: [], dependsOn: [], decision: null, createdAt: NOW, createdById: USER.id, updatedAt: NOW, rev: 1,
    ...overrides,
  };
}

// Gemeinsame Tests der Fragen-Repositories für jede Umsetzung von BoardDatabase (In-Memory und SQL Server)
export function questionRepositoryContract(name: string, create: () => Promise<BoardDatabase>, reset: () => Promise<void> = async () => {}) {
  describe(`Fragen-Vertrag: ${name}`, () => {
    let db: BoardDatabase;

    beforeEach(async () => {
      await reset();
      db = await create();
      await db.transaction((u) => u.users.upsert(USER));
    });
    afterEach(() => db.close());

    // Projekt ARC mit zwei Boards und je einem Ticket
    async function seed() {
      return db.transaction(async (u) => {
        const project = await u.projects.insert({ key: 'ARC', name: 'Arches', createdAt: NOW, createdBy: USER.id });
        const board = await u.boards.insert({ projectId: project.id, ownerId: USER.id, name: 'Sprint', repoPath: null, createdAt: NOW });
        const other = await u.boards.insert({ projectId: project.id, ownerId: USER.id, name: 'Anderes', repoPath: null, createdAt: NOW });
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId: project.id, boardId: board.id, number: 1 }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0002', projectId: project.id, boardId: other.id, number: 2 }));
        return { projectId: project.id, boardId: board.id, otherId: other.id };
      });
    }

    // IDs der Fragen zu einem Filter
    const ids = async (filter: BoardQuestionFilter) => (await db.transaction((u) => u.questions.list(filter))).map((q) => q.id);

    it('vergibt Fragenummern je Projekt fortlaufend, unabhängig von Ticketnummern', async () => {
      const { projectId } = await seed();
      const numbers = await db.transaction(async (u) => [await u.projects.takeQuestionNumber(projectId), await u.projects.takeQuestionNumber(projectId)]);
      expect(numbers).toEqual([1, 2]);
      expect(await db.transaction((u) => u.projects.byKey('ARC'))).toMatchObject({ nextQuestionNumber: 3, nextTicketNumber: 1 });
    });

    it('speichert eine Frage mit Tags, Tickets, Abhängigkeiten und Entscheidung', async () => {
      const { projectId } = await seed();
      const question = questionFixture({
        id: 'ARC-F0001', projectId, number: 1, title: 'Umlaute äöü', description: '# Text', occasion: 'Grill', originSession: 's-1',
        tags: ['speicher', 'ui'], ticketIds: ['ARC-0001', 'ARC-0002'], dependsOn: ['ARC-F0002', 'ARC-0001'],
      });
      await db.transaction(async (u) => {
        await u.questions.insert(questionFixture({ id: 'ARC-F0002', projectId, number: 2 }));
        await u.questions.insert(question);
      });
      const loaded = await db.transaction((u) => u.questions.byId('ARC-F0001'));
      expect(loaded).toEqual(question);
      const decided: BoardQuestion = {
        ...question, state: 'answered', rev: 2, updatedAt: LATER,
        decision: { outcome: 'answered', option: 'O2', text: null, reason: 'passt', byId: USER.id, byName: 'Florian', at: LATER },
      };
      await db.transaction((u) => u.questions.update(decided));
      expect(await db.transaction((u) => u.questions.byId('ARC-F0001'))).toEqual(decided);
      expect(await db.transaction((u) => u.questions.byId('ARC-F0099'))).toBeNull();
    });

    it('lehnt ein Update mit veralteter Version als conflict ab', async () => {
      const { projectId } = await seed();
      await db.transaction((u) => u.questions.insert(questionFixture({ id: 'ARC-F0001', projectId, number: 1 })));
      await db.transaction(async (u) => {
        const q = (await u.questions.byId('ARC-F0001', true))!;
        await u.questions.update({ ...q, title: 'Neu', rev: 2 });
      });
      await expect(db.transaction(async (u) => {
        const q = (await u.questions.byId('ARC-F0001'))!;
        await u.questions.update({ ...q, title: 'Zu spät', rev: 2 });
      })).rejects.toMatchObject({ code: 'conflict' });
      expect((await db.transaction((u) => u.questions.byId('ARC-F0001')))?.title).toBe('Neu');
    });

    it('filtert nach Projekt, IDs, betroffenen Tickets und Zuständen', async () => {
      const { projectId } = await seed();
      const other = await db.transaction((u) => u.projects.insert({ key: 'OTH', name: 'Other', createdAt: NOW, createdBy: USER.id }));
      await db.transaction(async (u) => {
        await u.questions.insert(questionFixture({ id: 'ARC-F0001', projectId, number: 1, ticketIds: ['ARC-0001'] }));
        await u.questions.insert(questionFixture({ id: 'ARC-F0002', projectId, number: 2, state: 'discarded', ticketIds: ['ARC-0001'] }));
        await u.questions.insert(questionFixture({ id: 'OTH-F0001', projectId: other.id, number: 1 }));
      });
      expect(await ids({ projectId })).toEqual(['ARC-F0001', 'ARC-F0002']);
      expect(await ids({ ids: ['OTH-F0001'] })).toEqual(['OTH-F0001']);
      expect(await ids({ ticketIds: ['ARC-0001'], states: ['open'] })).toEqual(['ARC-F0001']);
      expect(await ids({ ticketIds: [] })).toEqual([]);
      expect(await ids({})).toHaveLength(3);
    });

    it('speichert Recherchen mit Optionen, Kommentare und Verlauf in Reihenfolge', async () => {
      const { projectId } = await seed();
      await db.transaction(async (u) => {
        await u.questions.insert(questionFixture({ id: 'ARC-F0001', projectId, number: 1 }));
        await u.questionResearch.insert({
          questionId: 'ARC-F0001', at: NOW, byId: USER.id, byName: 'claude', kind: 'session', context: 'Kontext',
          options: [
            { id: 'O1', title: 'Datei', text: 'JSON', pros: ['einfach'], cons: [], recommended: false },
            { id: 'O2', title: 'DB', text: 'SQL', pros: [], cons: ['Aufwand', 'Schema'], recommended: true },
          ],
        });
        await u.questionResearch.insert({
          questionId: 'ARC-F0001', at: LATER, byId: USER.id, byName: 'claude', kind: 'subagent', context: '',
          options: [{ id: 'O3', title: 'Cache', text: 'Speicher', pros: [], cons: [], recommended: false }],
        });
        await u.questionComments.insert({ questionId: 'ARC-F0001', authorId: USER.id, authorName: 'Florian', authorKind: 'human', body: 'Hallo äöü', createdAt: NOW });
        await u.questionHistory.insert([
          { questionId: 'ARC-F0001', at: NOW, byId: USER.id, byName: 'claude', byKind: 'session', type: 'created', text: 'Frage angelegt' },
          { questionId: 'ARC-F0001', at: LATER, byId: USER.id, byName: 'Florian', byKind: null, type: 'ticket-deleted', text: 'Ticket ARC-0009 gelöscht' },
        ]);
      });
      const research = await db.transaction((u) => u.questionResearch.list('ARC-F0001'));
      expect(research.map((r) => [r.kind, r.options.map((o) => o.id)])).toEqual([['session', ['O1', 'O2']], ['subagent', ['O3']]]);
      expect(research[0]).toMatchObject({
        id: expect.any(Number), questionId: 'ARC-F0001', context: 'Kontext', byName: 'claude', at: NOW,
        options: [expect.objectContaining({ pros: ['einfach'], recommended: false }), expect.objectContaining({ cons: ['Aufwand', 'Schema'], recommended: true })],
      });
      expect(await db.transaction((u) => u.questionResearch.summaries(['ARC-F0001', 'ARC-F0009']))).toEqual({ 'ARC-F0001': { count: 2, optionCount: 1 } });
      expect(await db.transaction((u) => u.questionResearch.summaries([]))).toEqual({});
      expect(await db.transaction((u) => u.questionComments.list('ARC-F0001'))).toEqual([
        expect.objectContaining({ id: expect.any(Number), body: 'Hallo äöü', authorKind: 'human', authorName: 'Florian', createdAt: NOW }),
      ]);
      expect((await db.transaction((u) => u.questionHistory.list('ARC-F0001'))).map((h) => [h.type, h.byKind, h.text])).toEqual([
        ['created', 'session', 'Frage angelegt'], ['ticket-deleted', null, 'Ticket ARC-0009 gelöscht'],
      ]);
    });

    it('entfernt Bezüge auf ein Ticket und liefert die betroffenen Fragen', async () => {
      const { projectId } = await seed();
      await db.transaction(async (u) => {
        await u.questions.insert(questionFixture({ id: 'ARC-F0001', projectId, number: 1, ticketIds: ['ARC-0001'] }));
        await u.questions.insert(questionFixture({ id: 'ARC-F0002', projectId, number: 2, dependsOn: ['ARC-0001', 'ARC-0002'] }));
        await u.questions.insert(questionFixture({ id: 'ARC-F0003', projectId, number: 3, ticketIds: ['ARC-0002'] }));
      });
      // Lesen allein ändert nichts; es liefert dieselben Fragen wie das Entfernen
      expect(await db.transaction((u) => u.questions.idsReferencingTicket('ARC-0001'))).toEqual(['ARC-F0001', 'ARC-F0002']);
      expect(await db.transaction((u) => u.questions.idsReferencingTicket('ARC-0009'))).toEqual([]);
      expect((await db.transaction((u) => u.questions.byId('ARC-F0002')))!.dependsOn).toEqual(['ARC-0001', 'ARC-0002']);
      expect(await db.transaction((u) => u.questions.removeTicket('ARC-0001'))).toEqual(['ARC-F0001', 'ARC-F0002']);
      const after = await db.transaction((u) => u.questions.list({ projectId }));
      expect(after.map((q) => [q.id, q.ticketIds, q.dependsOn, q.rev])).toEqual([
        ['ARC-F0001', [], [], 1], ['ARC-F0002', [], ['ARC-0002'], 1], ['ARC-F0003', ['ARC-0002'], [], 1],
      ]);
    });

    it('ändert den Fragen-Kennwert des Projekts und den Board-Kennwert nur bei verknüpften Tickets', async () => {
      const { projectId, boardId, otherId } = await seed();
      const q0 = await db.questionMarker(projectId);
      const b0 = await db.changeMarker(boardId);
      const o0 = await db.changeMarker(otherId);
      await db.transaction((u) => u.questions.insert(questionFixture({ id: 'ARC-F0001', projectId, number: 1 })));
      const q1 = await db.questionMarker(projectId);
      expect(q1).not.toBe(q0);
      expect(await db.changeMarker(boardId)).toBe(b0);
      await db.transaction(async (u) => {
        const q = (await u.questions.byId('ARC-F0001', true))!;
        await u.questions.update({ ...q, ticketIds: ['ARC-0001'], rev: 2 });
      });
      const b1 = await db.changeMarker(boardId);
      expect(b1).not.toBe(b0);
      expect(await db.questionMarker(projectId)).not.toBe(q1);
      expect(await db.changeMarker(otherId)).toBe(o0);
      await db.transaction(async (u) => {
        const q = (await u.questions.byId('ARC-F0001', true))!;
        await u.questions.update({ ...q, state: 'answered', rev: 3 });
      });
      expect(await db.changeMarker(boardId)).not.toBe(b1);
      expect(await db.changeMarker(otherId)).toBe(o0);
      // Ohne Änderung bleibt der Kennwert gleich (auch nach einem reinen Lesezugriff)
      const stable = await db.questionMarker(projectId);
      await db.transaction((u) => u.questions.list({ projectId }));
      expect(await db.questionMarker(projectId)).toBe(stable);
    });

    it('verweigert das Löschen eines Tickets mit verknüpfter Frage, bis die Verknüpfung entfernt ist', async () => {
      const { projectId } = await seed();
      await db.transaction((u) => u.questions.insert(questionFixture({ id: 'ARC-F0001', projectId, number: 1, ticketIds: ['ARC-0001'] })));
      await expect(db.transaction((u) => u.tickets.delete('ARC-0001'))).rejects.toThrow();
      expect(await db.transaction((u) => u.tickets.byId('ARC-0001'))).not.toBeNull();
      await db.transaction((u) => u.questions.removeTicket('ARC-0001'));
      await db.transaction((u) => u.tickets.delete('ARC-0001'));
      expect(await db.transaction((u) => u.tickets.byId('ARC-0001'))).toBeNull();
    });
  });
}
