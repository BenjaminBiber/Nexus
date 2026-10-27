import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';

describe('Übernehmen und Freigeben', () => {
  it('übernimmt: setzt Zuweisung und In Arbeit, schreibt „übernommen“', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    const t = await f.tickets.claim('ARC-0001', 'claude', f.actor);
    expect(t).toMatchObject({ assignee: 'claude', status: 'in_progress' });
    expect((await f.tickets.get('ARC-0001')).history.slice(1).map((h) => [h.field, h.note])).toEqual([['status', 'übernommen'], ['assignee', 'übernommen']]);
  });

  it('lehnt die Übernahme eines zugewiesenen Tickets ab, auch für denselben Agenten; nur force übergeht es', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.tickets.claim('ARC-0001', 'claude-1', f.actor);
    await expect(f.tickets.claim('ARC-0001', 'claude-2', f.actor)).rejects.toMatchObject({ code: 'conflict' });
    await expect(f.tickets.claim('ARC-0001', 'claude-1', f.actor)).rejects.toMatchObject({ code: 'conflict' });
    expect((await f.tickets.claim('ARC-0001', 'claude-1', f.actor, { force: true })).assignee).toBe('claude-1');
    expect((await f.tickets.claim('ARC-0001', 'claude-2', f.actor, { force: true })).assignee).toBe('claude-2');
  });

  it('lehnt die Übernahme bei offener Voraussetzung ab, auch mit anderem Zielstatus', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'B', dependsOn: ['ARC-0001'] }, f.actor);
    await expect(f.tickets.claim('ARC-0002', 'claude', f.actor, { status: 'planned' })).rejects.toMatchObject({ code: 'dependency-open' });
  });

  it('lehnt die Übernahme mit veralteter Version ab', async () => {
    const f = await BoardFixture.create();
    const created = await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.tickets.update('ARC-0001', { title: 'B' }, f.actor);
    await expect(f.tickets.claim('ARC-0001', 'claude', f.actor, { rev: created.rev })).rejects.toMatchObject({ code: 'conflict' });
    expect((await f.tickets.get('ARC-0001')).assignee).toBeNull();
    expect((await f.tickets.claim('ARC-0001', 'claude', f.actor, { rev: created.rev + 1 })).assignee).toBe('claude');
  });

  it('verlangt einen Agentennamen', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await expect(f.tickets.claim('ARC-0001', ' ', f.actor)).rejects.toMatchObject({ code: 'agent-missing' });
  });

  it('gibt frei und ändert den Status nur auf Wunsch', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.tickets.claim('ARC-0001', 'claude', f.actor);
    expect(await f.tickets.release('ARC-0001', f.actor)).toMatchObject({ assignee: null, status: 'in_progress' });
    expect((await f.tickets.release('ARC-0001', f.actor, { status: 'planned' })).status).toBe('planned');
  });
});
