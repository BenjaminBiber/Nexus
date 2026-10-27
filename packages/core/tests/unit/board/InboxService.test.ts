import { describe, expect, it } from 'vitest';
import { InboxService } from '../../../src/board/inbox/InboxService.js';
import { USER } from './helpers/BoardDatabaseContract.js';
import { BoardFixture } from './helpers/BoardFixture.js';

const OTHER = { id: '22222222-2222-2222-2222-222222222222', name: 'Kollege' };

// Fixture mit eigenem Board „Sprint“ (ARC-0001…) und fremdem Board des Kollegen im selben Projekt
async function setup() {
  const f = await BoardFixture.create();
  const projectId = (await f.db.transaction((u) => u.projects.byKey('ARC')))!.id;
  const foreign = await f.db.transaction(async (u) => {
    await u.users.upsert(OTHER);
    return u.boards.insert({ projectId, ownerId: OTHER.id, name: 'Fremd', repoPath: null, createdAt: '2026-10-13T10:00:00.000Z' });
  });
  const blocked = async (title: string, reason: string, boardId = f.boardId) => {
    const t = await f.tickets.create(boardId, { title }, f.actor);
    await f.tickets.update(t.id, { status: 'blocked', blockedReason: reason }, f.actor);
    return t.id;
  };
  return { f, foreign, blocked, inbox: new InboxService(f.db) };
}

describe('InboxService.list', () => {
  it('liefert je Art einen Eintrag eigener Boards, nach Dringlichkeit sortiert', async () => {
    const { f, foreign, blocked, inbox } = await setup();
    await blocked('Fertig', 'Human Interaction: ansehen');
    f.advance(1);
    await blocked('Zugang', 'Zugang fehlt');
    f.advance(1);
    await blocked('Merge', 'Merge: Konflikt');
    f.advance(1);
    await blocked('Setup', 'Einrichtung fehlgeschlagen: npm ci');
    await f.tickets.create(f.boardId, { title: 'Planen', status: 'in_planning' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Läuft', status: 'in_progress' }, f.actor);
    await blocked('Fremd blockiert', 'Merge: fremd', foreign.id);
    const note = await f.notes.add(f.boardId, { title: 'Cache prüfen', body: 'Details dazu' }, f.actor);
    const taken = await f.notes.add(f.boardId, { title: 'Schon übernommen' }, f.actor);
    await f.notes.take(taken.id, f.actor);

    const items = await inbox.list(USER.id);
    expect(items.map((i) => [i.kind, i.title])).toEqual([
      ['merge', 'Merge'], ['setup', 'Setup'], ['accept', 'Fertig'], ['blocked', 'Zugang'], ['planning', 'Planen'], ['note', 'Cache prüfen'],
    ]);
    expect(items[0]).toMatchObject({ key: 'ticket:ARC-0003', ticketId: 'ARC-0003', boardId: f.boardId, boardName: 'Sprint', projectKey: 'ARC', detail: 'Merge: Konflikt' });
    expect(items[5]).toMatchObject({ key: `note:${note.id}`, noteId: note.id, detail: 'Details dazu', ticketId: null });
  });

  it('zeigt ein Ticket mit offener Frage nur über die Frage und zählt die wartenden Tickets', async () => {
    const { f, blocked, inbox } = await setup();
    await blocked('Wartet A', 'Zugang fehlt');
    await f.tickets.create(f.boardId, { title: 'Wartet B', status: 'in_planning' }, f.actor);
    const done = await f.tickets.create(f.boardId, { title: 'Fertig' }, f.actor);
    await f.tickets.move(done.id, 'done', f.actor, { acceptance: 'ok' });
    await f.questions.create('ARC', { title: 'Welcher Speicher?', ticketIds: ['ARC-0001', 'ARC-0002', 'ARC-0003'], occasion: 'Planung' }, f.agent);

    const items = await inbox.list(USER.id);
    expect(items).toEqual([expect.objectContaining({
      key: 'question:ARC-F0001', kind: 'question', title: 'Welcher Speicher?', detail: 'Planung', questionId: 'ARC-F0001',
      waitingTickets: 2, boardId: f.boardId,
    })]);
  });

  it('übergeht Fragen, die den Benutzer nicht betreffen, und entschiedene Fragen', async () => {
    const { f, foreign, inbox } = await setup();
    const colleague = { userId: OTHER.id, name: 'Kollege', kind: 'human' as const, session: null };
    await f.tickets.create(foreign.id, { title: 'Fremd' }, f.actor);
    await f.questions.create('ARC', { title: 'Fremde Frage', ticketIds: ['ARC-0001'] }, colleague);
    await f.questions.create('ARC', { title: 'Selbst gestellt' }, f.agent);
    await f.questions.create('ARC', { title: 'Verworfen' }, f.agent);
    await f.questions.discard('ARC-F0003', 'doppelt', f.human);

    expect((await inbox.list(USER.id)).map((i) => [i.key, i.boardId])).toEqual([['question:ARC-F0002', null]]);
  });

  it('filtert nach Board und lehnt fremde Boards ab', async () => {
    const { f, foreign, blocked, inbox } = await setup();
    const second = await f.boards.create('ARC', 'Zweites', f.actor);
    await blocked('Eins', 'Zugang fehlt');
    await blocked('Zwei', 'Zugang fehlt', second.id);
    await f.notes.add(second.id, { title: 'Notiz zwei' }, f.actor);

    expect((await inbox.list(USER.id)).map((i) => i.title)).toEqual(['Eins', 'Zwei', 'Notiz zwei']);
    expect((await inbox.list(USER.id, { boardId: second.id })).map((i) => i.title)).toEqual(['Zwei', 'Notiz zwei']);
    await expect(inbox.list(USER.id, { boardId: foreign.id })).rejects.toMatchObject({ code: 'board-not-owned' });
  });

  it('liefert ohne eigene Boards eine leere Liste und zählt je Art', async () => {
    const { blocked, inbox } = await setup();
    expect(await inbox.list(OTHER.id)).toEqual([]);
    await blocked('Eins', 'Zugang fehlt');
    const list = inbox.summarize(await inbox.list(USER.id));
    expect(list.total).toBe(1);
    expect(list.counts).toEqual({ merge: 0, setup: 0, question: 0, accept: 0, blocked: 1, planning: 0, note: 0 });
  });
});
