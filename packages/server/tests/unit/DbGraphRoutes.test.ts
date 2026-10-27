import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

describe('DbGraphRoutes', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let boardId: number;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    await backend.services.projects.create('ARC', 'Arches', backend.actor());
    boardId = (await backend.services.boards.create('ARC', 'Sprint', backend.actor())).id;
    const actor = backend.actor();
    await backend.services.tickets.create(boardId, { title: 'A', spec: '001-login' }, actor);
    await backend.services.tickets.create(boardId, { title: 'B', dependsOn: ['ARC-0001'], spec: '001-login' }, actor);
    await backend.services.tickets.create(boardId, { title: 'Allein' }, actor);
  });
  afterEach(() => server.dispose());

  it('liefert den Graph für Ticket, Board und Spec', async () => {
    const ticket = await server.get('/api/db/graph?ticket=arc-2');
    expect(ticket.status).toBe(200);
    expect(ticket.json.scope).toEqual({ kind: 'ticket', id: 'ARC-0002' });
    expect(ticket.json.edges).toEqual([{ from: 'ARC-0001', to: 'ARC-0002' }]);
    expect(ticket.json.criticalPath).toEqual(['ARC-0001', 'ARC-0002']);
    expect(ticket.json.nodes[1]).toMatchObject({ id: 'ARC-0002', column: 'waiting', boardName: 'Sprint', external: false });

    const board = await server.get(`/api/db/graph?board=${boardId}`);
    expect(board.json.hiddenWithoutEdges).toBe(1);
    expect(board.json.nodes.map((n: { id: string }) => n.id)).toEqual(['ARC-0001', 'ARC-0002']);

    const spec = await server.get('/api/db/graph?project=ARC&spec=001-login&done=0');
    expect(spec.json.scope).toEqual({ kind: 'spec', project: 'ARC', spec: '001-login' });
  });

  it('prüft die Parameter', async () => {
    expect((await server.get('/api/db/graph')).json.code).toBe('input-invalid');
    expect((await server.get(`/api/db/graph?ticket=ARC-0001&board=${boardId}`)).json.code).toBe('input-invalid');
    expect((await server.get('/api/db/graph?project=ARC')).json.code).toBe('input-invalid');
    expect((await server.get('/api/db/graph?board=abc')).json.code).toBe('id-invalid');
    expect((await server.get('/api/db/graph?ticket=ARC-0099')).status).toBe(404);
    expect((await server.get('/api/db/graph?board=999')).status).toBe(404);
  });

  it('liefert den Fortschritt der Specs eines Boards', async () => {
    const reply = await server.get(`/api/db/boards/${boardId}/spec-progress`);
    expect(reply.status).toBe(200);
    expect(reply.json).toEqual([{ spec: '001-login', total: 2, done: 0, blocked: 0, waiting: 1, inProgress: 0, open: 1, notNeeded: 0 }]);
    expect((await server.get('/api/db/boards/999/spec-progress')).status).toBe(404);
  });
});
