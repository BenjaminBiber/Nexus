import { describe, expect, it } from 'vitest';
import { BoardViewService } from '../../../src/board/BoardViewService.js';
import { BoardFixture } from './helpers/BoardFixture.js';

// ARC-0001 „Login“ (mit Abnahme) und ARC-0002 „Export“; offene Frage ARC-F0001 betrifft ARC-0001
async function setup(): Promise<BoardFixture> {
  const f = await BoardFixture.create();
  await f.tickets.create(f.boardId, { title: 'Login', acceptance: 'geht' }, f.actor);
  await f.tickets.create(f.boardId, { title: 'Export' }, f.actor);
  await f.questions.create('ARC', { title: 'Welcher Speicher?', ticketIds: ['ARC-0001'] }, f.agent);
  return f;
}

describe('Offene Fragen sperren ihre Tickets', () => {
  it('zeigt offene Fragen an Ticket und Board-Ansicht; das Ticket bleibt in seiner Spalte', async () => {
    const f = await setup();
    expect(await f.tickets.get('ARC-0001')).toMatchObject({
      column: 'backlog', waiting: false, waitingOnQuestions: [{ id: 'ARC-F0001', title: 'Welcher Speicher?' }],
    });
    expect((await f.tickets.get('ARC-0002')).waitingOnQuestions).toEqual([]);
    const view = await new BoardViewService(f.db, f.clock).get(f.boardId);
    expect(view.columns[0].tickets.map((t) => [t.id, t.waitingOnQuestions.map((q) => q.id)])).toEqual([['ARC-0001', ['ARC-F0001']], ['ARC-0002', []]]);
    expect((await f.tickets.list({ boardIds: [f.boardId] })).find((t) => t.id === 'ARC-0001')?.waitingOnQuestions).toHaveLength(1);
  });

  it('sperrt In Arbeit, Erledigt und Übernehmen außer mit force; nach der Entscheidung ist das Ticket frei', async () => {
    const f = await setup();
    await expect(f.tickets.move('ARC-0001', 'in_progress', f.actor)).rejects.toMatchObject({ code: 'question-open', message: expect.stringContaining('ARC-F0001') });
    // Die Meldung bleibt neutral; den --force-Hinweis ergänzt erst die CLI
    await expect(f.tickets.move('ARC-0001', 'in_progress', f.actor)).rejects.toMatchObject({ message: expect.not.stringContaining('--force') });
    await expect(f.tickets.move('ARC-0001', 'done', f.actor)).rejects.toMatchObject({ code: 'question-open' });
    await expect(f.tickets.claim('ARC-0001', 'claude', f.actor)).rejects.toMatchObject({ code: 'question-open' });
    await expect(f.tickets.update('ARC-0001', { status: 'in_progress' }, f.actor)).rejects.toMatchObject({ code: 'question-open' });
    expect((await f.tickets.move('ARC-0001', 'planned', f.actor)).status).toBe('planned');
    expect((await f.tickets.claim('ARC-0001', 'claude', f.actor, { force: true })).status).toBe('in_progress');
    await f.tickets.release('ARC-0001', f.actor, { status: 'planned' });
    await f.questions.answer('ARC-F0001', { text: 'Datei', reason: 'reicht' }, f.human);
    expect((await f.tickets.get('ARC-0001')).waitingOnQuestions).toEqual([]);
    expect((await f.tickets.claim('ARC-0001', 'claude', f.actor)).status).toBe('in_progress');
  });

  it('überspringt Tickets mit offener Frage bei next-planning; „Rückfrage“ ist kein Sonderfall mehr', async () => {
    const f = await setup();
    await f.tickets.create(f.boardId, { title: 'Blockiert mit Rückfrage', status: 'blocked' }, f.actor);
    await f.tickets.update('ARC-0003', { blockedReason: 'Rückfrage' }, f.actor);
    expect((await f.planning.next([f.boardId], null, f.actor))?.id).toBe('ARC-0002');
    await f.questions.setTickets('ARC-F0001', ['ARC-0001', 'ARC-0002'], f.agent);
    expect((await f.planning.next([f.boardId], 'claude', f.actor))?.id).toBe('ARC-0003');
  });

  it('entfernt beim Löschen eines Tickets dessen Bezüge an Fragen und schreibt den Verlauf der Frage', async () => {
    const f = await setup();
    await f.questions.create('ARC', { title: 'Zweite', dependsOn: ['ARC-0001'] }, f.agent);
    await f.tickets.delete('ARC-0001', f.actor);
    const first = await f.questions.get('ARC-F0001');
    expect(first).toMatchObject({ ticketIds: [], rev: 2 });
    expect(first.history.at(-1)).toMatchObject({ type: 'ticket-deleted', text: 'Ticket ARC-0001 „Login“ gelöscht, Bezug entfernt.', byKind: null, byName: 'claude' });
    expect(await f.questions.get('ARC-F0002')).toMatchObject({ dependsOn: [], ready: true });
  });

  it('entfernt Bezüge auch beim Löschen eines ganzen Boards', async () => {
    const f = await setup();
    await f.boards.delete(f.boardId, { deleteOpen: true }, f.actor);
    expect((await f.questions.get('ARC-F0001')).ticketIds).toEqual([]);
  });

  it('räumt beim Löschen eines Boards ohne offene Tickets Bezüge und Abhängigkeiten erledigter Tickets auf', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'Fertig', status: 'done', acceptance: 'geht' }, f.actor);
    await f.questions.create('ARC', { title: 'Betrifft', ticketIds: ['ARC-0001'] }, f.agent);
    await f.questions.create('ARC', { title: 'Hängt ab', dependsOn: ['ARC-0001'] }, f.agent);
    await f.boards.delete(f.boardId, { deleteOpen: false }, f.actor);
    const first = await f.questions.get('ARC-F0001');
    expect(first).toMatchObject({ ticketIds: [], rev: 2 });
    expect(first.history.at(-1)).toMatchObject({ type: 'ticket-deleted', text: 'Ticket ARC-0001 „Fertig“ gelöscht, Bezug entfernt.' });
    expect(await f.questions.get('ARC-F0002')).toMatchObject({ dependsOn: [], rev: 2 });
  });
});
