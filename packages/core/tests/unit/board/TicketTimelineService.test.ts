import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';
import { TicketTimelineService } from '../../../src/board/TicketTimelineService.js';

const names = { dispatcherName: 'dispatcher', agentPrefix: 'junior-' };

describe('TicketTimelineService', () => {
  it('liefert alle Ereignisse eines Tickets in Zeitfolge mit Person oder Agent', async () => {
    const f = await BoardFixture.create();
    const human = { userId: f.actor.userId, name: 'Florian' };
    await f.tickets.create(f.boardId, { title: 'A', steps: [{ text: 'Test', done: false }] }, human);
    f.advance(1);
    await f.planning.plan('ARC-0001', { plan: 'Plan' }, f.actor);
    f.advance(1);
    await f.tickets.claim('ARC-0001', 'junior-ARC-0001', f.actor);
    f.advance(1);
    await f.tickets.setStep('ARC-0001', 1, true, f.actor);
    f.advance(1);
    await f.comments.add('ARC-0001', 'Zwischenstand', human);
    f.advance(1);
    await f.questions.create('ARC', { title: 'Welcher Speicher?', ticketIds: ['ARC-0001'] }, f.agent);
    f.advance(1);
    await f.questions.answer('ARC-F0001', { text: 'SQL', reason: 'vorhanden' }, f.human);

    const timeline = await new TicketTimelineService(f.db, names).get('arc-0001');
    expect(timeline.ticketId).toBe('ARC-0001');
    const events = timeline.events;
    expect(events.map((e) => e.at)).toEqual([...events.map((e) => e.at)].sort());
    const types = events.map((e) => e.type);
    expect(types[0]).toBe('field:status');
    expect(events[0]).toMatchObject({ kind: 'status', note: 'angelegt', actor: { name: 'Florian', type: 'person' } });
    expect(events.find((e) => e.type === 'field:plan')).toMatchObject({ kind: 'content', actor: { name: 'claude', type: 'agent' } });
    expect(events.find((e) => e.type === 'field:assignee')).toMatchObject({ kind: 'claim', to: 'junior-ARC-0001' });
    expect(events.find((e) => e.type === 'field:step')).toMatchObject({ kind: 'content' });
    expect(events.find((e) => e.type === 'comment')).toMatchObject({ kind: 'comment', text: 'Zwischenstand' });
    expect(events.find((e) => e.type === 'question:created')).toMatchObject({
      kind: 'question', actor: { type: 'agent' }, ref: { questionId: 'ARC-F0001' }, text: expect.stringContaining('Welcher Speicher?'),
    });
    expect(events.find((e) => e.type === 'question:decided')).toMatchObject({
      kind: 'question', actor: { name: 'Florian', type: 'person' }, ref: { questionId: 'ARC-F0001' },
    });
    // Der Zustandswechsel der Frage steht schon über die Frage in der Zeitleiste, nicht doppelt aus dem Ticketverlauf
    expect(types.filter((t) => t === 'field:question')).toEqual([]);
  });

  it('behält Frage-Einträge des Ticketverlaufs, wenn die Frage nicht mehr verknüpft ist', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'B' }, f.actor);
    await f.questions.create('ARC', { title: 'Q', ticketIds: ['ARC-0001'] }, f.agent);
    await f.questions.answer('ARC-F0001', { text: 'ja', reason: 'r' }, f.human);
    await f.questions.setTickets('ARC-F0001', ['ARC-0002'], f.human);
    const events = (await new TicketTimelineService(f.db, null).get('ARC-0001')).events;
    expect(events.find((e) => e.type === 'field:question')).toMatchObject({ kind: 'question', ref: { questionId: 'ARC-F0001' } });
    expect(events.some((e) => e.type === 'question:created')).toBe(false);
  });

  it('meldet ein unbekanntes Ticket', async () => {
    const f = await BoardFixture.create();
    await expect(new TicketTimelineService(f.db, null).get('ARC-0099')).rejects.toMatchObject({ code: 'ticket-not-found' });
  });
});
