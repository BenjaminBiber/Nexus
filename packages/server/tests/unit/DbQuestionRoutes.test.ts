import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { MEMORY_USER, type MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

const AGENT = { userId: MEMORY_USER.id, name: 'claude', kind: 'session' as const, session: 's-1' };

describe('DbQuestionRoutes', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    await backend.services.projects.create('ARC', 'Arches', backend.actor());
    const boardId = (await backend.services.boards.create('ARC', 'Sprint', backend.actor())).id;
    await backend.services.tickets.create(boardId, { title: 'Login' }, backend.actor());
  });
  afterEach(() => server.dispose());

  it('stellt eine Frage als Mensch und listet die Fragen des Projekts mit Zählern', async () => {
    const created = await server.post('/api/db/projects/ARC/questions', { title: 'Welcher Speicher?', ticketIds: ['ARC-0001'], tags: ['speicher'], occasion: 'Board' });
    expect(created.status).toBe(201);
    expect(created.json).toMatchObject({
      id: 'ARC-F0001', originKind: 'human', originName: 'Florian', originSession: null, ticketIds: ['ARC-0001'],
      tickets: [{ id: 'ARC-0001', title: 'Login' }], tags: ['speicher'], occasion: 'Board', rev: 1,
    });
    expect((await server.get('/api/db/projects/arc/questions')).json).toMatchObject({
      counts: { open: 1, answered: 0, discarded: 0, mine: 1 }, questions: [{ id: 'ARC-F0001', ready: true, mine: true }],
    });
    expect((await server.get('/api/db/projects/ARC/questions?state=answered')).json.questions).toEqual([]);
    expect((await server.get('/api/db/projects/ARC/questions?ticket=ARC-0001&q=speicher')).json.questions).toHaveLength(1);
    expect((await server.get('/api/db/projects/ARC/questions?state=quatsch')).json.code).toBe('input-invalid');
    for (const bad of ['ticket=A&ticket=B', 'tag=a&tag=b', 'q=a&q=b', 'state=open&state=all']) {
      const res = await server.get(`/api/db/projects/ARC/questions?${bad}`);
      expect([res.status, res.json.code]).toEqual([422, 'input-invalid']);
    }
    expect((await server.get('/api/db/projects/XXX/questions')).status).toBe(404);
    expect((await server.post('/api/db/projects/ARC/questions', { title: ' ' })).json.code).toBe('title-empty');
    expect((await server.post('/api/db/projects/ARC/questions', {})).json.code).toBe('input-invalid');
    expect((await server.get('/api/db/questions/ARC-F0001')).json).toMatchObject({
      id: 'ARC-F0001', research: [], comments: [], history: [expect.objectContaining({ type: 'created', byKind: 'human' })],
    });
    expect((await server.get('/api/db/questions/ARC-F0009')).status).toBe(404);
    expect((await server.get('/api/db/tickets/ARC-0001')).json.waitingOnQuestions).toEqual([{ id: 'ARC-F0001', title: 'Welcher Speicher?' }]);
  });

  it('kennzeichnet serverseitig eigene Fragen (mine) und zählt offene eigene in counts.mine', async () => {
    const other = { userId: '22222222-2222-2222-2222-222222222222', name: 'Kollege', kind: 'human' as const, session: null };
    await backend.db.transaction((u) => u.users.upsert({ id: other.userId, name: other.name }));
    await backend.services.questions.create('ARC', { title: 'Zu meinem Ticket', ticketIds: ['ARC-0001'] }, other);
    await backend.services.questions.create('ARC', { title: 'Selbst gestellt' }, AGENT);
    await backend.services.questions.create('ARC', { title: 'Fremd' }, other);
    const list = (await server.get('/api/db/projects/ARC/questions')).json as { counts: Record<string, number>; questions: Array<{ id: string; mine: boolean }> };
    expect(list.counts).toEqual({ open: 3, answered: 0, discarded: 0, mine: 2 });
    expect(Object.fromEntries(list.questions.map((q) => [q.id, q.mine]))).toEqual({ 'ARC-F0001': true, 'ARC-F0002': true, 'ARC-F0003': false });
  });

  it('ändert, verknüpft und kommentiert mit Version und meldet Konflikte mit dem aktuellen Stand', async () => {
    await server.post('/api/db/projects/ARC/questions', { title: 'A' });
    expect((await server.patch('/api/db/questions/ARC-F0001', { rev: 1, title: 'B', occasion: null })).json).toMatchObject({ title: 'B', rev: 2 });
    const stale = await server.patch('/api/db/questions/ARC-F0001', { rev: 1, title: 'Alt' });
    expect(stale.status).toBe(409);
    expect(stale.json).toMatchObject({ code: 'conflict', current: { id: 'ARC-F0001', title: 'B', rev: 2 } });
    expect((await server.patch('/api/db/questions/ARC-F0001', { title: 'ohne' })).json.code).toBe('rev-missing');
    expect((await server.put('/api/db/questions/ARC-F0001/tickets', { rev: 2, ticketIds: ['ARC-0001'] })).json).toMatchObject({ ticketIds: ['ARC-0001'], rev: 3 });
    expect((await server.put('/api/db/questions/ARC-F0001/tickets', { rev: 3 })).json.code).toBe('input-invalid');
    expect((await server.put('/api/db/questions/ARC-F0001/tickets', { rev: 1, ticketIds: [] })).status).toBe(409);
    expect((await server.put('/api/db/questions/ARC-F0001/depends', { rev: 3, dependsOn: ['ARC-0001'] })).json).toMatchObject({ dependsOn: ['ARC-0001'], ready: false, rev: 4 });
    expect((await server.put('/api/db/questions/ARC-F0001/depends', { rev: 4, dependsOn: ['ARC-F0001'] })).json.code).toBe('dependency-self');
    const commented = await server.post('/api/db/questions/ARC-F0001/comments', { body: 'Hallo' });
    expect(commented.status).toBe(201);
    expect(commented.json.comments).toEqual([expect.objectContaining({ body: 'Hallo', authorKind: 'human', authorName: 'Florian' })]);
    expect((await server.post('/api/db/questions/ARC-F0001/comments', { body: ' ' })).json.code).toBe('comment-empty');
  });

  it('entscheidet, verwirft und öffnet wieder als Mensch; veraltete Version ergibt 409', async () => {
    await server.post('/api/db/projects/ARC/questions', { title: 'A', ticketIds: ['ARC-0001'] });
    await backend.services.questions.addResearch('ARC-F0001', { options: [{ title: 'Datei', text: 'JSON' }] }, AGENT);
    const stale = await server.post('/api/db/questions/ARC-F0001/decision', { rev: 1, option: 'O1', reason: 'x' });
    expect(stale.status).toBe(409);
    expect(stale.json.current.rev).toBe(2);
    expect((await server.post('/api/db/questions/ARC-F0001/decision', { rev: 2, option: 'O9' })).json.code).toBe('decision-option-unknown');
    const answered = await server.post('/api/db/questions/ARC-F0001/decision', { rev: 2, option: 'O1', reason: 'reicht' });
    expect(answered.json).toMatchObject({ state: 'answered', rev: 3, decision: { option: 'O1', byName: 'Florian' } });
    expect((await server.get('/api/db/tickets/ARC-0001')).json.waitingOnQuestions).toEqual([]);
    expect((await server.post('/api/db/questions/ARC-F0001/reopen', { rev: 3, reason: 'neu' })).json).toMatchObject({ state: 'open', rev: 4 });
    expect((await server.post('/api/db/questions/ARC-F0001/discard', { rev: 4 })).json.code).toBe('decision-reason-missing');
    expect((await server.post('/api/db/questions/ARC-F0001/discard', { rev: 4, reason: 'doppelt' })).json).toMatchObject({ state: 'discarded', rev: 5 });
  });
});
