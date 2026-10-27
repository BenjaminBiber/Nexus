import { describe, expect, it } from 'vitest';
import { QuestionConcern } from '../../../src/board/QuestionConcern.js';
import type { BoardQuestionActor } from '../../../src/board/BoardQuestionTypes.js';
import { USER } from './helpers/BoardDatabaseContract.js';
import { BoardFixture } from './helpers/BoardFixture.js';

const OTHER = { id: '22222222-2222-2222-2222-222222222222', name: 'Kollege' };
const colleague: BoardQuestionActor = { userId: OTHER.id, name: 'Kollege', kind: 'human', session: null };

// Eigenes Board mit Ticket ARC-0001, fremdes Board mit Ticket ARC-0002
async function setup(): Promise<BoardFixture> {
  const f = await BoardFixture.create();
  const projectId = (await f.db.transaction((u) => u.projects.byKey('ARC')))!.id;
  const foreign = await f.db.transaction(async (u) => {
    await u.users.upsert(OTHER);
    return u.boards.insert({ projectId, ownerId: OTHER.id, name: 'Fremd', repoPath: null, createdAt: '2026-10-13T10:00:00.000Z' });
  });
  await f.tickets.create(f.boardId, { title: 'Eigen' }, f.actor);
  await f.tickets.create(foreign.id, { title: 'Fremd' }, f.actor);
  return f;
}

describe('QuestionConcern', () => {
  const concern = new QuestionConcern();

  it('betrifft den Benutzer, wenn er sie gestellt hat oder ein Ticket auf einem eigenen Board liegt', () => {
    const own = new Set([1]);
    expect(concern.concerns({ createdById: USER.id, boardIds: [] }, USER.id, own)).toBe(true);
    expect(concern.concerns({ createdById: OTHER.id, boardIds: [2, 1] }, USER.id, own)).toBe(true);
    expect(concern.concerns({ createdById: OTHER.id, boardIds: [2] }, USER.id, own)).toBe(false);
    expect(concern.concerns({ createdById: OTHER.id, boardIds: [] }, USER.id, own)).toBe(false);
  });
});

describe('BoardQuestionService: eigene Fragen', () => {
  it('kennzeichnet in der Liste eigene Fragen und zählt offene eigene in counts.mine', async () => {
    const f = await setup();
    await f.questions.create('ARC', { title: 'Zu meinem Ticket', ticketIds: ['ARC-0001'] }, colleague);
    await f.questions.create('ARC', { title: 'Selbst gestellt' }, f.agent);
    await f.questions.create('ARC', { title: 'Zum fremden Ticket', ticketIds: ['ARC-0002'] }, colleague);
    await f.questions.create('ARC', { title: 'Fremd ohne Ticket' }, colleague);
    await f.questions.create('ARC', { title: 'Eigen, verworfen' }, f.agent);
    await f.questions.discard('ARC-F0005', 'doppelt', f.human);

    const list = await f.questions.list('ARC', {}, USER.id);
    expect(Object.fromEntries(list.questions.map((q) => [q.id, q.mine]))).toEqual({
      'ARC-F0001': true, 'ARC-F0002': true, 'ARC-F0003': false, 'ARC-F0004': false, 'ARC-F0005': true,
    });
    expect(list.counts).toEqual({ open: 4, answered: 0, discarded: 1, mine: 2 });
    const other = await f.questions.list('ARC', {}, OTHER.id);
    expect(other.counts.mine).toBe(3);
    // Ohne Betrachter ist nichts eigen
    expect((await f.questions.list('ARC')).counts.mine).toBe(0);
  });

  it('meldet (notifiable) auch selbst gestellte offene Fragen ohne Ticket, keine fremden', async () => {
    const f = await setup();
    await f.questions.create('ARC', { title: 'Selbst gestellt' }, f.agent);
    await f.questions.create('ARC', { title: 'Fremd ohne Ticket' }, colleague);
    await f.questions.create('ARC', { title: 'Zum fremden Ticket', ticketIds: ['ARC-0002'] }, colleague);
    expect((await f.questions.notifiable(USER.id)).map((q) => q.id)).toEqual(['ARC-F0001']);
    expect((await f.questions.notifiable(OTHER.id)).map((q) => q.id)).toEqual(['ARC-F0002', 'ARC-F0003']);
  });

  it('meldet selbst gestellte Fragen auch ohne eigenes Board', async () => {
    const f = await setup();
    const stranger: BoardQuestionActor = { userId: '33333333-3333-3333-3333-333333333333', name: 'Neu', kind: 'session', session: 's' };
    await f.db.transaction((u) => u.users.upsert({ id: stranger.userId, name: 'Neu' }));
    await f.questions.create('ARC', { title: 'Ohne Board' }, stranger);
    expect((await f.questions.notifiable(stranger.userId)).map((q) => q.id)).toEqual(['ARC-F0001']);
  });
});
