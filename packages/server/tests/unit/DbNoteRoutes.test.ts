import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

describe('DbNoteRoutes', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let boardId: number;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    await backend.services.projects.create('ARC', 'Arches', backend.actor());
    boardId = (await backend.services.boards.create('ARC', 'Sprint', backend.actor())).id;
  });
  afterEach(() => server.dispose());

  it('legt Notizen an, filtert nach Zustand, übernimmt und verwirft', async () => {
    const a = await server.post(`/api/db/boards/${boardId}/notes`, { title: 'Cache prüfen', body: 'Details', labels: ['perf'] });
    expect(a.status).toBe(201);
    expect(a.json).toMatchObject({ title: 'Cache prüfen', body: 'Details', labels: ['perf'], seenCount: 1, state: 'open', authorName: 'Florian' });
    const b = await server.post(`/api/db/boards/${boardId}/notes`, { title: 'Unwichtig' });
    const taken = await server.post(`/api/db/notes/${a.json.id}/take`, { title: 'Cache prüfen' });
    expect(taken.status).toBe(200);
    expect(taken.json).toMatchObject({ note: { state: 'taken', ticketId: 'ARC-0001' }, ticket: { id: 'ARC-0001', title: 'Cache prüfen', labels: ['perf'] } });
    expect((await server.post(`/api/db/notes/${b.json.id}/drop`, { reason: ' ' })).json.code).toBe('reason-missing');
    expect((await server.post(`/api/db/notes/${b.json.id}/drop`, { reason: 'doppelt' })).json).toMatchObject({ state: 'dropped', reason: 'doppelt' });
    expect((await server.get(`/api/db/boards/${boardId}/notes`)).json).toEqual([]);
    expect((await server.get(`/api/db/boards/${boardId}/notes?state=taken`)).json).toHaveLength(1);
    expect((await server.get(`/api/db/boards/${boardId}/notes?state=all`)).json).toHaveLength(2);
    expect((await server.get(`/api/db/boards/${boardId}/notes?state=constructor`)).json.code).toBe('input-invalid');
    expect((await server.post('/api/db/notes/999/take', {})).status).toBe(404);
  });

  it('lehnt einen schon notierten Befund mit Kandidaten ab, legt mit force an und zählt mit seen hoch', async () => {
    const first = await server.post(`/api/db/boards/${boardId}/notes`, { title: 'Dispatcher legt den Worktree-Branch doppelt an' });
    const again = { title: 'Worktree-Branch wird vom Dispatcher doppelt angelegt' };
    const rejected = await server.post(`/api/db/boards/${boardId}/notes`, again);
    expect(rejected.status).toBe(422);
    expect(rejected.json).toMatchObject({ code: 'note-similar', details: { notes: [{ id: first.json.id, state: 'open' }] } });
    expect((await server.post(`/api/db/boards/${boardId}/notes`, { ...again, force: true })).status).toBe(201);
    expect((await server.post(`/api/db/notes/${first.json.id}/seen`, {})).json).toMatchObject({ seenCount: 2 });
    expect((await server.post(`/api/db/boards/${boardId}/notes`, { body: 'ohne Titel' })).json.code).toBe('input-invalid');
  });

  it('nimmt eine Herkunft nur als existierendes Ticket an', async () => {
    expect((await server.post(`/api/db/boards/${boardId}/notes`, { title: 'x', source: 'ARC-0042' })).status).toBe(404);
  });
});
