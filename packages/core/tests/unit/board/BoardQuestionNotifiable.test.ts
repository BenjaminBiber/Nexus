import { describe, expect, it } from 'vitest';
import { USER } from './helpers/BoardDatabaseContract.js';
import { BoardFixture } from './helpers/BoardFixture.js';

const OTHER = { id: '22222222-2222-2222-2222-222222222222', name: 'Kollege' };

describe('BoardQuestionService.notifiable', () => {
  it('liefert nur offene eigene Fragen (hier: zu Tickets auf eigenen Boards), mit Recherchen und Repo-Pfaden', async () => {
    const colleague = { userId: OTHER.id, name: 'Kollege', kind: 'human' as const, session: null };
    const f = await BoardFixture.create();
    await f.boards.setRepoPath(f.boardId, 'C:\\Projects\\Arches');
    const projectId = (await f.db.transaction((u) => u.projects.byKey('ARC')))!.id;
    const foreign = await f.db.transaction(async (u) => {
      await u.users.upsert(OTHER);
      return u.boards.insert({ projectId, ownerId: OTHER.id, name: 'Fremd', repoPath: null, createdAt: '2026-10-13T10:00:00.000Z' });
    });
    await f.tickets.create(f.boardId, { title: 'Eigen' }, f.actor);
    await f.tickets.create(foreign.id, { title: 'Fremd' }, f.actor);
    await f.questions.create('ARC', { title: 'Eigene Frage', ticketIds: ['ARC-0001'], occasion: 'Planung' }, f.agent);
    await f.questions.addResearch('ARC-F0001', { options: [{ title: 'Datei', text: 'JSON', recommended: true }] }, f.agent);
    await f.questions.create('ARC', { title: 'Fremde Frage', ticketIds: ['ARC-0002'] }, colleague);
    await f.questions.create('ARC', { title: 'Ohne Ticket' }, colleague);
    await f.questions.create('ARC', { title: 'Verworfen', ticketIds: ['ARC-0001'] }, f.agent);
    await f.questions.discard('ARC-F0004', 'doppelt', f.human);

    expect(await f.questions.notifiable(USER.id)).toEqual([{
      id: 'ARC-F0001', title: 'Eigene Frage', occasion: 'Planung', origin: { kind: 'session' },
      research: [{ kind: 'session', options: [expect.objectContaining({ id: 'O1', recommended: true })] }],
      projectKey: 'ARC', projectName: 'Arches', repoPaths: ['C:\\Projects\\Arches'],
    }]);
    expect((await f.questions.notifiable(OTHER.id)).map((q) => q.id)).toEqual(['ARC-F0002', 'ARC-F0003']);
    expect(await f.questions.notifiable('33333333-3333-3333-3333-333333333333')).toEqual([]);
  });
});
