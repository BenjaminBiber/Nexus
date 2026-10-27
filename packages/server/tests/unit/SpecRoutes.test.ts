import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

describe('Spec-Routen', () => {
  let server: TestServer;
  let id: string;

  beforeEach(async () => {
    server = await TestServer.start();
    id = (await server.addProject()).id;
  });
  afterEach(() => server.dispose());

  it('kennt die Liste der Spec-Ordner nicht mehr', async () => {
    expect((await server.get(`/api/projects/${id}/specs`)).status).toBe(404);
  });

  it('meldet ein unbekanntes Projekt mit 404', async () => {
    expect((await server.get('/api/projects/gibtsnicht/spec-progress')).status).toBe(404);
  });

  it('liefert ohne Board-Konfiguration einen leeren Board-Fortschritt', async () => {
    const res = await server.get(`/api/projects/${id}/spec-progress`);
    expect(res.status).toBe(200);
    expect(res.json).toEqual([]);
  });
});

describe('GET …/spec-progress', () => {
  afterEach(async () => {
    await board?.server.dispose();
  });
  let board: { server: TestServer; backend: MemoryBoardBackend } | undefined;

  it('zählt die Specs des DB-Projekts, dessen Board zum Repo des Projekts gehört', async () => {
    board = await TestServer.startWithBoard();
    const { server, backend } = board;
    const added = await server.addProject();
    const actor = backend.actor();
    await backend.services.projects.create('ARC', 'Arches', actor);
    const boardId = (await backend.services.boards.create('ARC', 'Sprint', actor)).id;
    await backend.services.tickets.create(boardId, { title: 'A', spec: '001-login', status: 'blocked' }, actor);
    const pid = added.id;
    expect((await server.get(`/api/projects/${pid}/spec-progress`)).json).toEqual([]);

    await backend.services.boards.setRepoPath(boardId, added.project.root);
    expect((await server.get(`/api/projects/${pid}/spec-progress`)).json).toEqual([
      { spec: '001-login', total: 1, done: 0, blocked: 1, waiting: 0, inProgress: 0, open: 0, notNeeded: 0 },
    ]);
  });
});
