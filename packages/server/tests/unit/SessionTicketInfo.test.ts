import { describe, it, expect, afterEach } from 'vitest';
import { TokenStore } from '@harness/core';
import { TestServer } from '../helpers/TestServer.js';

describe('Ticketangaben an Sessions aus der Board-DB', () => {
  let server: TestServer;
  afterEach(() => server.dispose());

  // Meldet ein Projekt an und startet eine Session mit Tickets
  async function sessionWith(tickets: string[]): Promise<string> {
    const added = await server.addProject();
    const token = await new TokenStore(added.project.layout).read();
    const res = await server.post('/api/hook', { project: added.repo.path, event: { kind: 'session-start', sessionId: 's1', cwd: 'D:/wt', tickets } }, { 'x-harness-token': token });
    expect(res.status).toBe(204);
    return added.id;
  }

  it('liefert Titel und Status der DB-Tickets', async () => {
    const started = await TestServer.startWithBoard();
    server = started.server;
    const { backend } = started;
    await backend.services.projects.create('ARC', 'Arches', backend.actor());
    const board = await backend.services.boards.create('ARC', 'Sprint', backend.actor());
    await backend.services.tickets.create(board.id, { title: 'Login', status: 'planned' }, backend.actor());
    const id = await sessionWith(['ARC-0001', 'ARC-0099']);
    const [session] = (await server.get(`/api/projects/${id}/sessions`)).json;
    expect(session.tickets).toEqual(['ARC-0001', 'ARC-0099']);
    expect(session.ticketInfo).toEqual([
      { id: 'ARC-0001', title: 'Login', status: 'planned' },
      { id: 'ARC-0099', title: '(unbekannt)', status: 'backlog' },
    ]);
  });

  it('zeigt Sessions auch ohne eingerichtete Board-DB', async () => {
    server = await TestServer.start();
    const id = await sessionWith(['ARC-0001']);
    const [session] = (await server.get(`/api/projects/${id}/sessions`)).json;
    expect(session.ticketInfo).toEqual([{ id: 'ARC-0001', title: '(unbekannt)', status: 'backlog' }]);
  });
});
