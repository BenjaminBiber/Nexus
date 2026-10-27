import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { runFixture } from '../../../core/tests/unit/board/helpers/DispatchRunContract.js';
import { TempSqlite } from './helpers/TempSqlite.js';

describe('SQLite: Dispatcher-Läufe', () => {
  let temp: TempSqlite;
  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  it('speichert vollständig und idempotent', async () => {
    const r1 = runFixture({ id: 'ARC-0001-work-1', subjectId: 'ARC-0001', projectId: 1, boardId: 1, startedAt: '2026-10-01T00:00:00.000Z' });
    const r2 = runFixture({
      id: 'ARC-0002-work-1', subjectId: 'ARC-0002', projectId: 1, boardId: 2, startedAt: '2026-10-05T12:00:00.000Z', outcome: 'abort',
      abortCategory: 'timeout', abortReason: 'Zeitlimit äöü', costUsd: null, model: null, endedAt: null, durationMs: null, numTurns: null,
      exitCode: null, timedOut: true, inputTokens: null, outputTokens: null, cacheReadTokens: null, cacheWriteTokens: null, modelUsage: null,
      escalatedFrom: 'klein', attempt: 2,
    });
    const r3 = runFixture({ id: 'c42', subjectId: 'OTH-0001', projectId: 2, source: 'comments', startedAt: '2026-09-01T00:00:00.000Z' });

    const inserted = await temp.db.transaction(async (u) => [
      await u.dispatchRuns.insert(r1), await u.dispatchRuns.insert(r2), await u.dispatchRuns.insert(r3),
    ]);
    expect(inserted).toEqual([true, true, true]);
    expect(await temp.db.transaction((u) => u.dispatchRuns.insert({ ...r1, outcome: 'abort' }))).toBe(false);

    const all = await temp.db.transaction((u) => u.dispatchRuns.list({ projectId: 1, from: '2026-10-01T00:00:00.000Z', to: '2026-10-06T00:00:00.000Z' }));
    expect(all).toEqual([r1, r2]);
  });

  it('filtert Zeitraum und Board', async () => {
    const r1 = runFixture({ id: 'ARC-0001-work-1', subjectId: 'ARC-0001', projectId: 1, boardId: 1, startedAt: '2026-10-01T00:00:00.000Z' });
    const r2 = runFixture({ id: 'ARC-0002-work-1', subjectId: 'ARC-0002', projectId: 1, boardId: 2, startedAt: '2026-10-05T12:00:00.000Z' });
    await temp.db.transaction(async (u) => { await u.dispatchRuns.insert(r1); await u.dispatchRuns.insert(r2); });

    expect(await temp.db.transaction((u) => u.dispatchRuns.list({ projectId: 1, from: '2026-10-01T00:00:00.001Z', to: '2026-10-05T12:00:00.000Z' })))
      .toEqual([]);
    expect((await temp.db.transaction((u) => u.dispatchRuns.list({ projectId: 1, boardId: 2, from: '2026-01-01T00:00:00.000Z', to: '2027-01-01T00:00:00.000Z' })))
      .map((r) => r.id)).toEqual(['ARC-0002-work-1']);
    expect((await temp.db.transaction((u) => u.dispatchRuns.list({ projectId: 1, from: '2026-10-01T00:00:00Z', to: '2026-10-06T00:00:00Z' })))
      .map((r) => r.id)).toEqual(['ARC-0001-work-1', 'ARC-0002-work-1']);
  });

  it('liefert firstStarted je Projekt und Quelle', async () => {
    const r1 = runFixture({ id: 'ARC-0001-work-1', subjectId: 'ARC-0001', projectId: 1, startedAt: '2026-10-01T00:00:00.000Z' });
    const r2 = runFixture({ id: 'ARC-0002-work-1', subjectId: 'ARC-0002', projectId: 1, startedAt: '2026-10-05T12:00:00.000Z' });
    await temp.db.transaction(async (u) => { await u.dispatchRuns.insert(r1); await u.dispatchRuns.insert(r2); });

    expect(await temp.db.transaction((u) => u.dispatchRuns.firstStarted(1, 'dispatcher'))).toBe('2026-10-01T00:00:00.000Z');
    expect(await temp.db.transaction((u) => u.dispatchRuns.firstStarted(1, 'comments'))).toBeNull();
  });

  it('liefert existing in Blöcken', async () => {
    const r1 = runFixture({ id: 'ARC-0001-work-1', subjectId: 'ARC-0001', projectId: 1, startedAt: '2026-10-01T00:00:00.000Z' });
    const r2 = runFixture({ id: 'c42', subjectId: 'OTH-0001', projectId: 2, source: 'comments', startedAt: '2026-09-01T00:00:00.000Z' });
    await temp.db.transaction(async (u) => { await u.dispatchRuns.insert(r1); await u.dispatchRuns.insert(r2); });

    const found = await temp.db.transaction((u) => u.dispatchRuns.existing(['c42', 'c43', 'ARC-0001-work-1']));
    expect(found).toEqual(expect.arrayContaining(['c42', 'ARC-0001-work-1']));
    expect(found).toHaveLength(2);
    expect(await temp.db.transaction((u) => u.dispatchRuns.existing([]))).toEqual([]);

    const manyIds = Array.from({ length: 1200 }, (_, i) => `gen-${i}`);
    await temp.db.transaction(async (u) => { for (const id of manyIds) await u.dispatchRuns.insert(runFixture({ id, subjectId: 'ARC-0003', projectId: 1 })); });
    expect(await temp.db.transaction((u) => u.dispatchRuns.existing([...manyIds, 'c42']))).toEqual(expect.arrayContaining([...manyIds, 'c42']));
  });

  it('begrenzt Dauer und Abbruchgrund wie MSSQL', async () => {
    const run = runFixture({
      id: 'ARC-0001-work-1', subjectId: 'ARC-0001', projectId: 1, durationMs: 3_000_000_000, abortReason: 'x'.repeat(600),
    });
    await temp.db.transaction((u) => u.dispatchRuns.insert(run));
    const [saved] = await temp.db.transaction((u) => u.dispatchRuns.list({ projectId: 1, from: '2026-01-01T00:00:00.000Z', to: '2027-01-01T00:00:00.000Z' }));
    expect(saved.durationMs).toBeNull();
    expect(saved.abortReason).toHaveLength(500);
  });
});
