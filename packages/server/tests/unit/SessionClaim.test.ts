import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { TokenStore } from '@harness/core';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

describe('POST …/sessions/:sid/claim (DB-Board)', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let id: string;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    await backend.services.projects.create('ARC', 'Arches', backend.actor());
    const board = await backend.services.boards.create('ARC', 'Sprint', backend.actor());
    await backend.services.tickets.create(board.id, { title: 'Eins' }, backend.actor());
    const added = await server.addProject();
    id = added.id;
    const token = await new TokenStore(added.project.layout).read();
    await server.post('/api/hook', { project: added.repo.path, event: { kind: 'session-start', sessionId: 's1', cwd: 'D:/wt' } }, { 'x-harness-token': token });
  });
  afterEach(() => server.dispose());

  it('ordnet die Session dem DB-Ticket zu und verknüpft sie in der DB', async () => {
    const res = await server.post(`/api/projects/${id}/sessions/s1/claim`, { ticket: 'arc-1' });
    expect(res.status).toBe(200);
    expect(res.json.tickets).toEqual(['ARC-0001']);
    expect(res.json.ticketInfo).toEqual([{ id: 'ARC-0001', title: 'Eins', status: 'backlog' }]);
    expect((await backend.services.tickets.get('ARC-0001')).sessions).toEqual(['s1']);
  });

  it('meldet unbekannte Session und unbekanntes Ticket mit 404', async () => {
    const noSession = await server.post(`/api/projects/${id}/sessions/gibtsnicht/claim`, { ticket: 'ARC-0001' });
    expect(noSession.status).toBe(404);
    expect(noSession.json.code).toBe('session-not-found');
    expect((await backend.services.tickets.get('ARC-0001')).sessions).toEqual([]);
    const noTicket = await server.post(`/api/projects/${id}/sessions/s1/claim`, { ticket: 'ARC-0099' });
    expect(noTicket.status).toBe(404);
    expect(noTicket.json.code).toBe('ticket-not-found');
  });

  it('verlangt die Angabe des Tickets', async () => {
    expect((await server.post(`/api/projects/${id}/sessions/s1/claim`, {})).status).toBe(422);
  });

  it('übernimmt die Meldung der CLI (kanonische DB-ID) in die Sessions-Ansicht des Servers', async () => {
    const board = (await backend.services.tickets.get('ARC-0001')).boardId;
    await backend.services.tickets.create(board, { title: 'Zwei', status: 'planned' }, backend.actor());
    const res = await server.post(`/api/projects/${id}/sessions/s1/claim`, { ticket: 'ARC-0002' });
    expect(res.status).toBe(200);
    const [session] = (await server.get(`/api/projects/${id}/sessions`)).json;
    expect(session.tickets).toEqual(['ARC-0002']);
    expect(session.ticketInfo).toEqual([{ id: 'ARC-0002', title: 'Zwei', status: 'planned' }]);
  });
});
