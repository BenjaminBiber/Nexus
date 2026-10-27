import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { BoardDatabase } from '../../../../src/board/BoardDatabase.js';
import type { DispatchRun } from '../../../../src/board/DispatchRunTypes.js';
import { ticketFixture, USER } from './BoardDatabaseContract.js';

const NOW = '2026-10-13T10:00:00.000Z';

// Vollständiger Lauf mit Standardwerten
export function runFixture(over: Partial<DispatchRun> & Pick<DispatchRun, 'id' | 'subjectId' | 'projectId'>): DispatchRun {
  return {
    kind: 'work', boardId: null, agent: 'junior-X', attempt: 1, profile: 'standard', escalatedFrom: null, requestedModel: 'opus',
    model: 'claude-opus-5-5', effort: 'high', startedAt: NOW, endedAt: '2026-10-13T10:05:00.000Z', durationMs: 300_000, numTurns: 12,
    exitCode: 0, timedOut: false, outcome: 'done', abortCategory: null, abortReason: null, costUsd: 0.123456, inputTokens: 10,
    outputTokens: 2_000_000_000_000, cacheReadTokens: 30, cacheWriteTokens: 40, modelUsage: '{"claude-opus-5-5":{"costUsd":0.123456}}',
    source: 'dispatcher', maxTurns: null, timeoutMinutes: null, profileReason: null, ruleIndex: null, configHash: null, deniedCount: null,
    deniedTools: null, ...over,
  };
}

// Gemeinsame Tests der Lauf- und Auswertungs-Repositories (In-Memory und SQL Server)
export function dispatchRunContract(name: string, create: () => Promise<BoardDatabase>, reset: () => Promise<void> = async () => {}) {
  describe(`Dispatcher-Läufe: ${name}`, () => {
    let db: BoardDatabase;

    beforeEach(async () => {
      await reset();
      db = await create();
      await db.transaction((u) => u.users.upsert(USER));
    });
    afterEach(() => db.close());

    // Projekt ARC mit zwei Boards, Projekt OTH mit einem
    async function seed() {
      return db.transaction(async (u) => {
        const arc = await u.projects.insert({ key: 'ARC', name: 'Arches', createdAt: NOW, createdBy: USER.id });
        const oth = await u.projects.insert({ key: 'OTH', name: 'Andere', createdAt: NOW, createdBy: USER.id });
        const b1 = await u.boards.insert({ projectId: arc.id, ownerId: USER.id, name: 'Eins', repoPath: null, createdAt: NOW });
        const b2 = await u.boards.insert({ projectId: arc.id, ownerId: USER.id, name: 'Zwei', repoPath: null, createdAt: NOW });
        const b3 = await u.boards.insert({ projectId: oth.id, ownerId: USER.id, name: 'Drei', repoPath: null, createdAt: NOW });
        return { arc: arc.id, oth: oth.id, b1: b1.id, b2: b2.id, b3: b3.id };
      });
    }

    it('speichert Läufe vollständig, lehnt doppelte IDs ab und filtert nach Projekt, Board und Startzeit', async () => {
      const { arc, oth, b1, b2 } = await seed();
      const r1 = runFixture({ id: 'ARC-0001-work-1', subjectId: 'ARC-0001', projectId: arc, boardId: b1, startedAt: '2026-10-01T00:00:00.000Z' });
      const r2 = runFixture({
        id: 'ARC-0002-work-1', subjectId: 'ARC-0002', projectId: arc, boardId: b2, startedAt: '2026-10-05T12:00:00.000Z', outcome: 'abort',
        abortCategory: 'timeout', abortReason: 'Zeitlimit äöü', costUsd: null, model: null, endedAt: null, durationMs: null, numTurns: null,
        exitCode: null, timedOut: true, inputTokens: null, outputTokens: null, cacheReadTokens: null, cacheWriteTokens: null, modelUsage: null,
        escalatedFrom: 'klein', attempt: 2,
      });
      const r3 = runFixture({ id: 'c42', subjectId: 'OTH-0001', projectId: oth, source: 'comments', startedAt: '2026-09-01T00:00:00.000Z' });
      expect(await db.transaction(async (u) => [await u.dispatchRuns.insert(r1), await u.dispatchRuns.insert(r2), await u.dispatchRuns.insert(r3)]))
        .toEqual([true, true, true]);
      expect(await db.transaction((u) => u.dispatchRuns.insert({ ...r1, outcome: 'abort' }))).toBe(false);

      const all = await db.transaction((u) => u.dispatchRuns.list({ projectId: arc, from: '2026-10-01T00:00:00.000Z', to: '2026-10-06T00:00:00.000Z' }));
      expect(all).toEqual([r1, r2]);
      // Ende ausschließlich, Board-Filter
      expect(await db.transaction((u) => u.dispatchRuns.list({ projectId: arc, from: '2026-10-01T00:00:00.001Z', to: '2026-10-05T12:00:00.000Z' }))).toEqual([]);
      expect((await db.transaction((u) => u.dispatchRuns.list({ projectId: arc, boardId: b2, from: '2026-01-01T00:00:00.000Z', to: '2027-01-01T00:00:00.000Z' })))
        .map((r) => r.id)).toEqual(['ARC-0002-work-1']);

      expect(await db.transaction((u) => u.dispatchRuns.firstStarted(arc, 'dispatcher'))).toBe('2026-10-01T00:00:00.000Z');
      expect(await db.transaction((u) => u.dispatchRuns.firstStarted(arc, 'comments'))).toBeNull();
      expect(await db.transaction((u) => u.dispatchRuns.existing(['c42', 'c43', 'ARC-0001-work-1']))).toEqual(expect.arrayContaining(['c42', 'ARC-0001-work-1']));
      expect(await db.transaction((u) => u.dispatchRuns.existing([]))).toEqual([]);
    });

    it('liefert Läufe aller Projekte im Zeitraum, ältester zuerst', async () => {
      const { arc, oth } = await seed();
      const a = runFixture({ id: 'A', subjectId: 'ARC-0001', projectId: arc, startedAt: '2026-10-02T00:00:00.000Z' });
      const b = runFixture({ id: 'B', subjectId: 'OTH-0001', projectId: oth, startedAt: '2026-10-01T00:00:00.000Z' });
      const c = runFixture({ id: 'C', subjectId: 'OTH-0002', projectId: oth, startedAt: '2026-10-09T00:00:00.000Z' });
      await db.transaction(async (u) => { for (const r of [a, b, c]) await u.dispatchRuns.insert(r); });
      expect(await db.transaction((u) => u.dispatchRuns.listAll('2026-10-01T00:00:00.000Z', '2026-10-09T00:00:00.000Z'))).toEqual([b, a]);
    });

    it('gibt die Dashboard-Details eines Laufs unverändert zurück, gesetzt wie offen', async () => {
      const { arc, b1 } = await seed();
      const voll = runFixture({
        id: 'ARC-0010-work-1', subjectId: 'ARC-0010', projectId: arc, boardId: b1, startedAt: '2026-10-02T00:00:00.000Z',
        maxTurns: 250, timeoutMinutes: 90, profileReason: 'rule', ruleIndex: 3, configHash: 'a'.repeat(64), deniedCount: 2,
        deniedTools: 'Bash,WebFetch',
      });
      const leer = runFixture({ id: 'ARC-0011-work-1', subjectId: 'ARC-0011', projectId: arc, boardId: b1, startedAt: '2026-10-02T01:00:00.000Z' });
      await db.transaction(async (u) => [await u.dispatchRuns.insert(voll), await u.dispatchRuns.insert(leer)]);

      expect(await db.transaction((u) => u.dispatchRuns.list({ projectId: arc, from: '2026-10-02T00:00:00.000Z', to: '2026-10-03T00:00:00.000Z' })))
        .toEqual([voll, leer]);
    });

    it('findet Tickets mit letztem Wechsel nach Erledigt im Zeitraum, deren Statuswechsel und [dispatch]-Kommentare', async () => {
      const { arc, oth, b1, b2, b3 } = await seed();
      const status = (ticketId: string, at: string, to: string) => ({ ticketId, at, byId: USER.id, byName: 'x', field: 'status', from: null, to, note: null });
      await db.transaction(async (u) => {
        await u.tickets.insert(ticketFixture({ id: 'ARC-0001', projectId: arc, boardId: b1, number: 1, status: 'done' }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0002', projectId: arc, boardId: b2, number: 2, status: 'done' }));
        await u.tickets.insert(ticketFixture({ id: 'ARC-0003', projectId: arc, boardId: b1, number: 3, status: 'done' }));
        await u.tickets.insert(ticketFixture({ id: 'OTH-0001', projectId: oth, boardId: b3, number: 1, status: 'done' }));
        await u.history.insert([
          status('ARC-0001', '2026-10-02T10:00:00.000Z', 'done'),
          { ...status('ARC-0001', '2026-10-02T11:00:00.000Z', 'x'), field: 'title' },
          status('ARC-0002', '2026-10-03T10:00:00.000Z', 'done'),
          // ARC-0003: erst im Zeitraum, zuletzt danach erledigt – zählt nicht
          status('ARC-0003', '2026-10-03T10:00:00.000Z', 'done'),
          status('ARC-0003', '2026-10-04T10:00:00.000Z', 'in_progress'),
          status('ARC-0003', '2026-10-20T10:00:00.000Z', 'done'),
          status('OTH-0001', '2026-10-03T10:00:00.000Z', 'done'),
        ]);
        for (const body of ['[dispatch] Start: an junior-ARC-0001 übergeben', '[dispatch] Abbruch Versuch 1/3: x.', '[dispatch] Start: wieder', 'Mensch: [dispatch] Abbruch']) {
          await u.comments.insert({ ticketId: 'ARC-0001', authorId: USER.id, authorName: 'dispatcher', body, createdAt: NOW });
        }
        await u.comments.insert({ ticketId: 'OTH-0001', authorId: USER.id, authorName: 'dispatcher', body: '[dispatch] Start: an junior-OTH-0001', createdAt: NOW });
      });
      const range = { projectId: arc, from: '2026-10-01T00:00:00.000Z', to: '2026-10-10T00:00:00.000Z' };
      expect(await db.transaction((u) => u.dispatchStats.doneTicketIds(range))).toEqual(['ARC-0001', 'ARC-0002']);
      expect(await db.transaction((u) => u.dispatchStats.doneTicketIds({ ...range, boardId: b2 }))).toEqual(['ARC-0002']);
      const history = await db.transaction((u) => u.dispatchStats.statusHistory(['ARC-0001', 'ARC-0003']));
      expect(history.map((h) => [h.ticketId, h.to])).toEqual([['ARC-0001', 'done'], ['ARC-0003', 'done'], ['ARC-0003', 'in_progress'], ['ARC-0003', 'done']]);
      expect(await db.transaction((u) => u.dispatchStats.statusHistory([]))).toEqual([]);
      expect(await db.transaction((u) => u.dispatchStats.dispatchCommentCounts(['ARC-0001', 'ARC-0002']))).toEqual({ 'ARC-0001': { starts: 2, aborts: 1 } });
      expect(await db.transaction((u) => u.dispatchStats.dispatchCommentCounts([]))).toEqual({});
      const comments = await db.transaction((u) => u.dispatchStats.dispatchComments(arc));
      expect(comments.map((c) => c.body)).toEqual(['[dispatch] Start: an junior-ARC-0001 übergeben', '[dispatch] Abbruch Versuch 1/3: x.', '[dispatch] Start: wieder']);
    });

    it('legt jede Konfig-Version einmal an und liefert sie nach Zeitraum sortiert', async () => {
      const version = (hash: string, firstSeenAt: string, content = '{"defaultProfile":"standard"}') =>
        ({ hash, dispatcher: 'dispatcher', firstSeenAt, content });
      const alt = version('aaaa000000000001', '2026-10-01T08:00:00.000Z');
      const neu = version('bbbb000000000002', '2026-10-05T09:00:00.000Z', '{"defaultProfile":"klein","umlaute":"äöü"}');
      const spaet = version('cccc000000000003', '2026-10-20T09:00:00.000Z');
      expect(await db.transaction(async (u) => [
        await u.dispatchRuns.recordConfigVersion(alt),
        await u.dispatchRuns.recordConfigVersion(neu),
        await u.dispatchRuns.recordConfigVersion(spaet),
      ])).toEqual([true, true, true]);
      // Derselbe Hash ein zweites Mal: nichts geschrieben, der erste Zeitpunkt bleibt stehen
      expect(await db.transaction((u) => u.dispatchRuns.recordConfigVersion({ ...alt, firstSeenAt: '2026-10-09T00:00:00.000Z', dispatcher: 'anderer' })))
        .toBe(false);

      expect(await db.transaction((u) => u.dispatchRuns.listConfigVersions('2026-10-01T00:00:00.000Z', '2026-10-10T00:00:00.000Z')))
        .toEqual([alt, neu]);
      expect(await db.transaction((u) => u.dispatchRuns.listConfigVersions('2026-10-05T09:00:00.000Z', '2026-10-20T09:00:00.000Z')))
        .toEqual([neu]);
      expect(await db.transaction((u) => u.dispatchRuns.listConfigVersions('2027-01-01T00:00:00.000Z', '2027-02-01T00:00:00.000Z'))).toEqual([]);
    });
  });
}
