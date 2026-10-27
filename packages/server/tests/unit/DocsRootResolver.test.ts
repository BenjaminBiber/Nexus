import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BoardConfigStore, BoardRepoPaths } from '@harness/core';
import { BoardRuntimeHolder } from '../../src/dbboard/BoardRuntimeHolder.js';
import { DocsRootResolver } from '../../src/docs/DocsRootResolver.js';
import { MEMORY_CONFIG, MEMORY_USER, type MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

describe('DocsRootResolver', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let resolver: DocsRootResolver;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    resolver = new DocsRootResolver(new BoardRuntimeHolder(async () => backend), server.home);
  });
  afterEach(() => server.dispose());

  // Legt Projekt und Board an und liefert die Board-ID
  async function board(name = 'Sprint'): Promise<number> {
    if ((await server.get('/api/db/projects')).json.length === 0) await server.post('/api/db/projects', { key: 'ARC', name: 'Arches' });
    return (await server.post('/api/db/boards', { project: 'ARC', name })).json.id;
  }

  // Setzt den eigenen Repo-Pfad des Testbenutzers für ein Board
  const setRepo = (id: number, path: string) => new BoardRepoPaths(backend.repoStore, MEMORY_USER.id).set(id, path);

  it('liefert den Repo-Pfad des Boards, das zum Ordner gehört', async () => {
    const { repo } = await server.addProject();
    const id = await board();
    await setRepo(id, repo.path);
    expect(await resolver.resolve(repo.path)).toEqual({
      root: repo.path.replace(/\\/g, '/'),
      boardId: id,
      boardName: 'Sprint',
      reason: null,
    });
  });

  it('meldet no-board, wenn kein Board dem Ordner zugeordnet ist', async () => {
    const { repo } = await server.addProject();
    await board();
    expect(await resolver.resolve(repo.path)).toEqual({ root: null, boardId: null, boardName: null, reason: 'no-board' });
  });

  it('meldet no-repo-path mit dem aktiven Board, wenn diesem der Repo-Pfad fehlt', async () => {
    const { repo } = await server.addProject();
    const id = await board('Aktiv');
    await new BoardConfigStore(server.home).write({ ...MEMORY_CONFIG, activeBoard: id });
    expect(await resolver.resolve(repo.path)).toEqual({ root: null, boardId: id, boardName: 'Aktiv', reason: 'no-repo-path' });
  });

  it('meldet missing-folder, wenn der Repo-Pfad des Boards nicht mehr existiert', async () => {
    const { repo } = await server.addProject();
    const id = await board();
    await setRepo(id, repo.path);
    rmSync(repo.path, { recursive: true, force: true });
    expect(await resolver.resolve(repo.path)).toMatchObject({ root: null, boardId: id, reason: 'missing-folder' });
  });

  it('liefert mit board aus der Query das genannte Board, auch wenn es zu einem anderen Ordner gehört', async () => {
    const { repo } = await server.addProject();
    const { repo: other } = await server.addProject();
    const id = await board('Fremd');
    await setRepo(id, other.path);
    expect(await resolver.resolveFor(repo.path, String(id))).toEqual({
      root: other.path.replace(/\\/g, '/'),
      boardId: id,
      boardName: 'Fremd',
      reason: null,
    });
  });

  it('fällt ohne gültiges board auf das Board des Ordners zurück und meldet unbekannte Boards als no-board', async () => {
    const { repo } = await server.addProject();
    const id = await board();
    await setRepo(id, repo.path);
    expect((await resolver.resolveFor(repo.path, undefined)).boardId).toBe(id);
    expect((await resolver.resolveFor(repo.path, 'abc')).boardId).toBe(id);
    expect(await resolver.resolveFor(repo.path, '999')).toMatchObject({ root: null, reason: 'no-board' });
  });

  it('liest über die Docs-Route mit ?board= die Dokumente aus dem Repo des genannten Boards', async () => {
    const { id: pid } = await server.addProject();
    const { repo: other } = await server.addProject();
    mkdirSync(join(other.path, 'docs', 'superpowers', 'specs'), { recursive: true });
    writeFileSync(join(other.path, 'docs', 'superpowers', 'specs', '2026-10-01-fremd-design.md'), '# Fremd — Design-Spec\n');
    const id = await board('Fremd');
    await setRepo(id, other.path);
    const res = await server.get(`/api/projects/${pid}/docs?board=${id}`);
    expect(res.json.source).toMatchObject({ boardId: id, reason: null });
    expect(res.json.docs.map((d: { path: string }) => d.path)).toContain('docs/superpowers/specs/2026-10-01-fremd-design.md');
    const file = await server.get(`/api/projects/${pid}/docs/file?path=docs/superpowers/specs/2026-10-01-fremd-design.md&board=${id}`);
    expect(file.status).toBe(200);
  });

  it('meldet no-board ohne Board-Laufzeit', async () => {
    const { repo } = await server.addProject();
    expect(await new DocsRootResolver(null, null).resolve(repo.path)).toEqual({
      root: null, boardId: null, boardName: null, reason: 'no-board',
    });
  });
});
