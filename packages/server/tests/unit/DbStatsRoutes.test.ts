import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';
import { runFixture } from '../../../core/tests/unit/board/helpers/DispatchRunContract.js';

describe('DbStatsRoutes', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let boardId: number;
  let projectId: number;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    projectId = (await backend.services.projects.create('ARC', 'Arches', backend.actor())).id;
    boardId = (await backend.services.boards.create('ARC', 'Sprint', backend.actor())).id;
  });
  afterEach(() => server.dispose());

  it('liefert die Kennzahlen eines Projekts mit Filter, Zeitraum und Läufen', async () => {
    const actor = backend.actor();
    await backend.services.tickets.create(boardId, { title: 'A', status: 'planned' }, actor);
    await backend.services.tickets.move('ARC-0001', 'done', actor, { acceptance: 'ok' });
    const today = new Date();
    const day = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}-${String(today.getDate()).padStart(2, '0')}`;
    await backend.services.dispatchRuns.record(runFixture({ id: 'r1', subjectId: 'ARC-0001', projectId, boardId, startedAt: new Date().toISOString(), costUsd: 0.5 }));

    const reply = await server.get(`/api/db/stats/dispatch?project=arc&board=${boardId}&from=${day}&to=${day}`);
    expect(reply.status).toBe(200);
    expect(reply.json.filter).toEqual({ project: 'ARC', boardId, from: day, to: day, kind: 'work' });
    expect(reply.json.kpis).toMatchObject({ runs: 1, done: 1, successRate: 1 });
    expect(reply.json.byProfile).toEqual([expect.objectContaining({ profile: 'standard', runs: 1, done: 1 })]);

    // Standard-Zeitraum: 8 Wochen bis heute; andere Lauf-Art
    const plain = await server.get('/api/db/stats/dispatch?project=ARC&kind=merge');
    expect(plain.json.filter.to).toBe(day);
    expect(plain.json.filter.kind).toBe('merge');
    expect(plain.json.kpis.runs).toBe(0);

    // Seit Konfig-Änderung: ohne Fassung Standard-Zeitraum, sonst ab dem Tag der neuesten Fassung
    expect((await server.get('/api/db/stats/dispatch?project=ARC&from=config')).json.filter.from).toBe(plain.json.filter.from);
    const changed = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 3, 9);
    await backend.services.dispatchRuns.recordConfigVersion({ hash: 'h', dispatcher: 'd', firstSeenAt: changed.toISOString(), content: '{}' });
    const since = await server.get('/api/db/stats/dispatch?project=ARC&from=config');
    expect(since.json.filter.from).toBe(`${changed.getFullYear()}-${String(changed.getMonth() + 1).padStart(2, '0')}-${String(changed.getDate()).padStart(2, '0')}`);
    expect(since.json.configChangedAt).toBe(changed.toISOString());
  });

  it('prüft die Parameter', async () => {
    const other = (await backend.services.projects.create('OTH', 'Andere', backend.actor())).id;
    const foreign = (await backend.services.boards.create('OTH', 'Fremd', backend.actor())).id;
    expect(other).toBeGreaterThan(0);
    expect((await server.get('/api/db/stats/dispatch')).json.code).toBe('input-invalid');
    expect((await server.get('/api/db/stats/dispatch?project=XYZ')).status).toBe(404);
    expect((await server.get(`/api/db/stats/dispatch?project=ARC&board=${foreign}`)).json.code).toBe('input-invalid');
    expect((await server.get('/api/db/stats/dispatch?project=ARC&board=999')).status).toBe(404);
    expect((await server.get('/api/db/stats/dispatch?project=ARC&from=2026-02-01&to=2026-01-01')).json.code).toBe('input-invalid');
    expect((await server.get('/api/db/stats/dispatch?project=ARC&from=2026-02-30')).json.code).toBe('input-invalid');
    expect((await server.get('/api/db/stats/dispatch?project=ARC&from=2025-01-01&to=2026-01-02')).json.message).toMatch(/zu lang/);
    expect((await server.get('/api/db/stats/dispatch?project=ARC&from=2025-01-02&to=2026-01-02')).status).toBe(200);
    expect((await server.get('/api/db/stats/dispatch?project=ARC&kind=plan')).json.code).toBe('input-invalid');
  });
});

describe('DbStatsRoutes ohne Board-Datenbank', () => {
  it('antwortet mit 503 wie die anderen DB-Routen', async () => {
    const server = await TestServer.start();
    try {
      expect((await server.get('/api/db/stats/dispatch?project=ARC')).status).toBe(503);
    } finally {
      server.dispose();
    }
  });
});
