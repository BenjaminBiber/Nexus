import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

describe('GET /api/db/search', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    const s = backend.services;
    await s.projects.create('ARC', 'Arches', backend.actor());
    await s.projects.create('SV1', 'Service', backend.actor());
    const a = (await s.boards.create('ARC', 'Sprint', backend.actor())).id;
    const b = (await s.boards.create('SV1', 'Incidents', backend.actor())).id;
    await s.tickets.create(a, { title: 'Export bauen' }, backend.actor());
    await s.tickets.create(b, { title: 'Export hängt' }, backend.actor());
    await s.notes.add(a, { title: 'Export später prüfen' }, backend.actor());
  });
  afterEach(() => server.dispose());

  it('findet Treffer über Projekte und Boards mit Fundstelle', async () => {
    const res = await server.get('/api/db/search?q=export');
    expect(res.status).toBe(200);
    expect(res.json).toMatchObject({ query: 'export', terms: ['export'], truncated: false });
    expect(res.json.hits.map((h: { id: string }) => h.id)).toEqual(['SV1-0001', 'ARC-0001', expect.any(String)]);
    expect(res.json.hits[0]).toMatchObject({ kind: 'ticket', project: { key: 'SV1' }, board: { name: 'Incidents' }, snippet: 'Export hängt', matches: [[0, 6]] });
  });

  it('filtert nach Art und begrenzt die Zahl', async () => {
    expect((await server.get('/api/db/search?q=export&kinds=note')).json.hits.map((h: { kind: string }) => h.kind)).toEqual(['note']);
    const limited = await server.get('/api/db/search?q=export&limit=1');
    expect(limited.json.hits).toHaveLength(1);
    expect(limited.json.truncated).toBe(true);
    expect((await server.get('/api/db/search?q=export&kinds=quatsch')).json.code).toBe('input-invalid');
    expect((await server.get('/api/db/search?q=export&limit=abc')).json.code).toBe('input-invalid');
  });

  it('lehnt zu kurze und fehlende Eingaben mit 422 ab', async () => {
    const short = await server.get('/api/db/search?q=e');
    expect(short.status).toBe(422);
    expect(short.json.code).toBe('query-short');
    expect((await server.get('/api/db/search')).json.code).toBe('query-short');
  });
});

describe('GET /api/db/search ohne Board-Datenbank', () => {
  it('antwortet mit 503 und Hinweis', async () => {
    const server = await TestServer.start();
    try {
      const res = await server.get('/api/db/search?q=export');
      expect(res.status).toBe(503);
      expect(res.json.code).toBe('board-config-missing');
    } finally {
      await server.dispose();
    }
  });
});
