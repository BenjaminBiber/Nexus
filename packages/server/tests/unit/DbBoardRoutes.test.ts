import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { WindowsFolderPicker } from '../../src/dbboard/FolderPicker.js';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TempRepo } from '../../../core/tests/unit/helpers/TempRepo.js';
import { TestServer } from '../helpers/TestServer.js';

describe('DbBoardRoutes', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
  });
  afterEach(() => server.dispose());

  // Legt (falls nötig) Projekt ARC und ein Board an und liefert dessen ID
  async function seed(name = 'Sprint'): Promise<number> {
    if ((await server.get('/api/db/projects')).json.length === 0) await server.post('/api/db/projects', { key: 'ARC', name: 'Arches' });
    return (await server.post('/api/db/boards', { project: 'ARC', name })).json.id;
  }

  it('legt Projekte an, listet und benennt sie um', async () => {
    const created = await server.post('/api/db/projects', { key: 'arc', name: 'Arches' });
    expect(created.status).toBe(201);
    expect(created.json).toMatchObject({ key: 'ARC', name: 'Arches' });
    expect((await server.get('/api/db/projects')).json.map((p: { key: string }) => p.key)).toEqual(['ARC']);
    expect((await server.patch('/api/db/projects/ARC', { name: 'Arches 2' })).json).toMatchObject({ key: 'ARC', name: 'Arches 2' });
    expect((await server.post('/api/db/projects', { key: 'ARC', name: 'Doppelt' })).json.code).toBe('project-exists');
    expect((await server.patch('/api/db/projects/XYZ', { name: 'x' })).status).toBe(404);
    const missing = await server.post('/api/db/projects', { key: 'NEU' });
    expect(missing.status).toBe(422);
    expect(missing.json.code).toBe('input-invalid');
  });

  it('legt Boards an, filtert nach Projekt und ändert Name und Repo-Pfad', async () => {
    const id = await seed();
    await server.post('/api/db/projects', { key: 'OTH', name: 'Other' });
    expect((await server.post('/api/db/boards', { project: 'OTH', name: 'Fremd' })).status).toBe(201);
    expect((await server.get('/api/db/boards?project=ARC')).json).toEqual([
      expect.objectContaining({ id, name: 'Sprint', projectKey: 'ARC', ownerName: 'Florian', openTickets: 0 }),
    ]);
    expect((await server.get('/api/db/boards')).json).toHaveLength(2);
    expect((await server.patch(`/api/db/boards/${id}`, { name: 'Sprint 2', repoPath: 'C:/repo' })).json).toMatchObject({ id, name: 'Sprint 2', repoPath: 'C:/repo', projectKey: 'ARC' });
    expect((await server.patch(`/api/db/boards/${id}`, { repoPath: null })).json.repoPath).toBeNull();
    expect((await server.patch(`/api/db/boards/${id}`, {})).json.code).toBe('nothing-changed');
    expect((await server.patch('/api/db/boards/999', { name: 'x' })).status).toBe(404);
    expect((await server.get('/api/db/boards/abc/view')).json.code).toBe('id-invalid');
  });

  it('speichert „Mein Repo-Pfad“ lokal über core und zeigt keine fremden DB-Pfade', async () => {
    const id = await seed();
    const saved = await server.patch(`/api/db/boards/${id}`, { repoPath: 'C:/mein' });
    expect(saved.json).toMatchObject({ id, repoPath: 'C:/mein', repoSource: 'local' });
    expect(await backend.repoStore.read()).toEqual(new Map([[id, 'C:/mein']]));
    expect((await backend.db.transaction((u) => u.boards.byId(id)))?.repoPath).toBeNull();
    const projectId = (await backend.db.transaction((u) => u.projects.byKey('ARC')))!.id;
    const foreign = await backend.db.transaction(async (u) => {
      await u.users.upsert({ id: '22222222-2222-2222-2222-222222222222', name: 'Kollege' });
      return u.boards.insert({ projectId, ownerId: '22222222-2222-2222-2222-222222222222', name: 'Fremd', repoPath: 'D:/kollege', createdAt: '2026-10-01T08:00:00.000Z' });
    });
    const listed = (await server.get('/api/db/boards')).json as Array<{ id: number; repoPath: string | null; repoSource: string | null }>;
    expect(listed.find((b) => b.id === foreign.id)).toMatchObject({ repoPath: null, repoSource: null });
    expect(JSON.stringify(listed)).not.toContain('D:/kollege');
  });

  it('meldet Benutzer und aktives Board und aktiviert ein Board in board.yaml', async () => {
    const id = await seed();
    expect((await server.get('/api/db/active')).json).toEqual({ user: { id: '11111111-1111-1111-1111-111111111111', name: 'Florian' }, boardId: null, board: null });
    const activated = await server.post(`/api/db/boards/${id}/activate`);
    expect(activated.status).toBe(200);
    expect(activated.json).toMatchObject({ boardId: id, board: { id, name: 'Sprint' } });
    expect(readFileSync(server.home.boardConfigFile, 'utf8')).toContain(`activeBoard: ${id}`);
    expect((await server.get('/api/db/active')).json.boardId).toBe(id);
    expect((await server.post('/api/db/boards/999/activate')).status).toBe(404);
  });

  it('löscht ein Board mit offenen Tickets nur mit deleteOpen und vergisst es als aktives Board', async () => {
    const id = await seed();
    await server.post(`/api/db/boards/${id}/activate`);
    await backend.services.tickets.create(id, { title: 'Offen' }, backend.actor());
    const refused = await server.delete(`/api/db/boards/${id}`);
    expect(refused.status).toBe(422);
    expect(refused.json.code).toBe('board-has-open-tickets');
    const deleted = await server.delete(`/api/db/boards/${id}?deleteOpen=true`);
    expect(deleted.status).toBe(200);
    expect(deleted.json).toEqual({ deletedTickets: ['ARC-0001'] });
    expect((await server.get('/api/db/active')).json.boardId).toBeNull();
    expect(readFileSync(server.home.boardConfigFile, 'utf8')).toContain('activeBoard: null');
  });

  it('löscht bei kaputter board.yaml nichts und antwortet mit 503', async () => {
    const id = await seed();
    writeFileSync(server.home.boardConfigFile, 'server: [kaputt');
    const res = await server.delete(`/api/db/boards/${id}`);
    expect(res.status).toBe(503);
    expect(res.json.code).toBe('board-config-broken');
    expect((await backend.services.boards.list()).map((b) => b.id)).toContain(id);
  });

  it('liefert die Board-Ansicht mit allen Spalten und Kommentarzahl', async () => {
    const id = await seed();
    const a = await backend.services.tickets.create(id, { title: 'A' }, backend.actor());
    await backend.services.tickets.create(id, { title: 'B', dependsOn: [a.id] }, backend.actor());
    await backend.services.comments.add(a.id, 'Hallo', backend.actor());
    const view = await server.get(`/api/db/boards/${id}/view`);
    expect(view.status).toBe(200);
    expect(view.json.boardId).toBe(id);
    expect(view.json.columns.map((c: { id: string }) => c.id)).toEqual(['backlog', 'waiting', 'in_planning', 'planned', 'in_progress', 'blocked', 'done', 'not_needed']);
    expect(view.json.columns[0].tickets).toEqual([expect.objectContaining({ id: 'ARC-0001', commentCount: 1 })]);
    expect(view.json.columns[1]).toMatchObject({ title: 'Wartet', tickets: [expect.objectContaining({ id: 'ARC-0002', dependsOpen: ['ARC-0001'] })] });
    expect((await server.get('/api/db/boards/999/view')).status).toBe(404);
  });
});

describe('DbBoardRoutes – Ordnerauswahl', () => {
  it('liefert den gewählten Ordner, gibt den Startordner weiter und meldet Abbruch als null', async () => {
    const starts: Array<string | null> = [];
    const answers: Array<string | null> = ['C:/Repos/Mein Repo', null];
    const server = await TestServer.start({ folderPicker: { pick: async (start) => (starts.push(start), answers.shift() ?? null) } });
    try {
      expect((await server.post('/api/db/pick-folder', { start: ' C:/Repos ' })).json).toEqual({ path: 'C:/Repos/Mein Repo' });
      expect((await server.post('/api/db/pick-folder', {})).json).toEqual({ path: null });
      expect(starts).toEqual(['C:/Repos', null]);
      expect((await server.post('/api/db/pick-folder', { start: 5 })).status).toBe(422);
    } finally {
      await server.dispose();
    }
  });

  it('meldet fehlende Ordnerauswahl mit verständlicher Meldung (kein Windows)', async () => {
    await expect(new WindowsFolderPicker('linux').pick(null)).rejects.toMatchObject({ code: 'folder-picker-unavailable' });
  });
});

describe('DbBoardRoutes ohne Board-Datenbank', () => {
  it('antwortet ohne board.yaml mit 503 und Hinweis', async () => {
    const server = await TestServer.start();
    try {
      const res = await server.get('/api/db/projects');
      expect(res.status).toBe(503);
      expect(res.json.code).toBe('board-config-missing');
      expect(res.json.message).toContain('Board-Datenbank');
    } finally {
      await server.dispose();
    }
  });

  it('antwortet bei kaputter board.yaml mit 503 board-config-broken', async () => {
    const server = await TestServer.start();
    try {
      mkdirSync(dirname(server.home.boardConfigFile), { recursive: true });
      writeFileSync(server.home.boardConfigFile, 'server: [kaputt');
      for (const url of ['/api/db/active', '/api/db/projects']) {
        const res = await server.get(url);
        expect(res.status, url).toBe(503);
        expect(res.json.code, url).toBe('board-config-broken');
      }
    } finally {
      await server.dispose();
    }
  });
});

describe('DbBoardRoutes – Branches des Repos', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let repo: TempRepo;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    repo = new TempRepo();
    repo.git('branch', '-M', 'main');
    repo.git('branch', 'release/1');
    await server.post('/api/db/projects', { key: 'ARC', name: 'Arches' });
  });
  afterEach(() => {
    server.dispose();
    repo.dispose();
  });

  it('liefert die Branches des Board-Repos', async () => {
    const id = (await server.post('/api/db/boards', { project: 'ARC', name: 'Sprint' })).json.id;
    await backend.services.boards.setRepoPath(id, repo.path);
    expect((await server.get(`/api/db/boards/${id}/branches`)).json).toEqual({ branches: ['main', 'release/1'], reason: null });
  });

  it('meldet ohne Repo-Pfad und bei fehlendem Ordner „no-repo“ statt eines Fehlers', async () => {
    const id = (await server.post('/api/db/boards', { project: 'ARC', name: 'Ohne' })).json.id;
    expect((await server.get(`/api/db/boards/${id}/branches`)).json).toEqual({ branches: [], reason: 'no-repo' });
    await backend.services.boards.setRepoPath(id, join(repo.base, 'fehlt'));
    expect((await server.get(`/api/db/boards/${id}/branches`)).json).toEqual({ branches: [], reason: 'no-repo' });
  });

  it('legt einen neuen Branch im Board-Repo an', async () => {
    const id = (await server.post('/api/db/boards', { project: 'ARC', name: 'Sprint' })).json.id;
    await backend.services.boards.setRepoPath(id, repo.path);
    const created = await server.post(`/api/db/boards/${id}/branches`, { name: 'release/2' });
    expect(created.status).toBe(201);
    expect(created.json).toEqual({ name: 'release/2', created: true, from: 'main' });
    expect((await server.get(`/api/db/boards/${id}/branches`)).json.branches).toEqual(['main', 'release/1', 'release/2']);
    expect((await server.post(`/api/db/boards/${id}/branches`, { name: 'release/2' })).json).toEqual({ name: 'release/2', created: false, from: null });
  });

  it('lehnt ungültige Namen und Boards ohne Repo ab', async () => {
    const id = (await server.post('/api/db/boards', { project: 'ARC', name: 'Ohne' })).json.id;
    expect((await server.post(`/api/db/boards/${id}/branches`, { name: 'release/3' })).json.code).toBe('board-repo-missing');
    await backend.services.boards.setRepoPath(id, repo.path);
    expect((await server.post(`/api/db/boards/${id}/branches`, { name: 'a b' })).json.code).toBe('target-branch-invalid');
    expect((await server.post(`/api/db/boards/${id}/branches`, { name: 'ticket/x' })).json.code).toBe('target-branch-invalid');
  });

  it('meldet ein unbekanntes Board als nicht gefunden', async () => {
    expect((await server.post('/api/db/boards/999/branches', { name: 'x' })).status).toBe(404);
    expect((await server.get('/api/db/boards/999/branches')).status).toBe(404);
    expect((await server.get('/api/db/boards/abc/branches')).json.code).toBe('id-invalid');
  });
});
