import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { TestServer } from '../helpers/TestServer.js';
import { TempRepo } from '../../../core/tests/unit/helpers/TempRepo.js';

describe('ProjectRoutes', () => {
  let server: TestServer;
  beforeEach(async () => (server = await TestServer.start()));
  afterEach(() => server.dispose());

  it('liefert zunächst eine leere Liste', async () => {
    const res = await server.get('/api/projects');
    expect(res.status).toBe(200);
    expect(res.json).toEqual([]);
  });

  it('meldet ein Projekt an: 201 neu, 200 bekannt', async () => {
    const { repo } = await server.addProject();
    const known = await server.post('/api/projects', { path: repo.path });
    expect(known.status).toBe(200);
    const other = new TempRepo();
    server.repos.push(other);
    const { HarnessProject } = await import('@harness/core');
    await HarnessProject.init(other.path);
    const created = await server.post('/api/projects', { path: other.path });
    expect(created.status).toBe(201);
    expect(created.json).toMatchObject({ name: 'Mein Projekt ä', reachable: true });
    expect((await server.get('/api/projects')).json).toHaveLength(2);
  });

  it('lehnt fehlenden, relativen und nicht eingerichteten Pfad mit 422 ab', async () => {
    expect((await server.post('/api/projects', {})).status).toBe(422);
    expect((await server.post('/api/projects', { path: 'relativ/pfad' })).status).toBe(422);
    const bare = new TempRepo();
    server.repos.push(bare);
    const res = await server.post('/api/projects', { path: bare.path });
    expect(res.status).toBe(422);
    expect(res.json.message).toContain('harness init');
  });

  it('markiert ein Projekt mit gelöschtem Ordner als nicht erreichbar', async () => {
    const { repo } = await server.addProject();
    repo.dispose();
    const list = (await server.get('/api/projects')).json;
    expect(list[0].reachable).toBe(false);
  });
});
