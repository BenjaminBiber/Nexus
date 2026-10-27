import { describe, expect, it } from 'vitest';
import { DispatchRunService } from '../../../src/board/DispatchRunService.js';
import { BoardFixture } from './helpers/BoardFixture.js';
import { runFixture } from './helpers/DispatchRunContract.js';

describe('DispatchRunService', () => {
  it('erfasst Läufe einmal und wertet Statuswechsel, Kommentare und Läufe eines Zeitraums aus', async () => {
    const f = await BoardFixture.create();
    const service = new DispatchRunService(f.db);
    const projectId = (await f.projects.list()).find((p) => p.key === 'ARC')!.id;
    await f.tickets.create(f.boardId, { title: 'A', status: 'planned', labels: ['ui'] }, f.actor);
    await f.tickets.create(f.boardId, { title: 'B', status: 'planned' }, f.actor);
    f.advance(60);
    await f.tickets.claim('ARC-0001', 'junior-ARC-0001', f.actor);
    await f.comments.add('ARC-0001', '[dispatch] Start: an junior-ARC-0001 übergeben', f.actor);
    f.advance(120);
    await f.tickets.move('ARC-0001', 'done', f.actor, { acceptance: 'ok' });

    const run = runFixture({ id: 'ARC-0001-work', subjectId: 'ARC-0001', projectId, boardId: f.boardId, startedAt: '2026-10-13T11:00:00.000Z', costUsd: 1.5 });
    expect(await service.record(run)).toBe(true);
    expect(await service.record(run)).toBe(false);

    const stats = await service.stats({ projectId, from: new Date('2026-10-12T00:00:00.000Z'), to: new Date('2026-10-14T00:00:00.000Z') });
    expect(stats.kpis).toMatchObject({ runs: 1, subjects: 1, done: 1, successRate: 1, firstTryRate: 1 });
    expect(stats.runsSince).toBe('2026-10-13T11:00:00.000Z');
    expect(stats.byProfile).toEqual([expect.objectContaining({ profile: 'standard', runs: 1, done: 1 })]);
    expect(stats.configChangedAt).toBeNull();
    // Andere Lauf-Art ohne Treffer
    expect((await service.stats({ projectId, from: new Date('2026-10-12'), to: new Date('2026-10-14'), kind: 'merge' })).kpis.runs).toBe(0);

    // Neueste Fassung der Einstellung vor dem Ende des Zeitraums
    await service.recordConfigVersion({ hash: 'a', dispatcher: 'd', firstSeenAt: '2026-10-01T08:00:00.000Z', content: '{}' });
    await service.recordConfigVersion({ hash: 'b', dispatcher: 'd', firstSeenAt: '2026-10-13T08:00:00.000Z', content: '{}' });
    await service.recordConfigVersion({ hash: 'c', dispatcher: 'd', firstSeenAt: '2026-10-20T08:00:00.000Z', content: '{}' });
    expect((await service.stats({ projectId, from: new Date('2026-10-12'), to: new Date('2026-10-14') })).configChangedAt).toBe('2026-10-13T08:00:00.000Z');
  });
});
