import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';

// Tickets von einem anderen Board desselben Projekts holen (eine Transaktion, alles oder nichts)
describe('TicketService.moveToBoard', () => {
  // Fixture mit zweitem Board „Kollege“ im Projekt ARC
  async function setup() {
    const f = await BoardFixture.create();
    const source = await f.boards.create('ARC', 'Kollege', f.actor);
    return { f, sourceId: source.id };
  }

  it('holt mehrere Tickets, hängt sie ans Ende ihrer Spalte und erhöht die Version', async () => {
    const { f, sourceId } = await setup();
    await f.tickets.create(f.boardId, { title: 'Schon da' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Auch da', status: 'planned' }, f.actor);
    await f.tickets.create(sourceId, { title: 'A' }, f.actor);
    await f.tickets.create(sourceId, { title: 'B' }, f.actor);
    await f.tickets.create(sourceId, { title: 'C', status: 'planned' }, f.actor);
    const moved = await f.tickets.moveToBoard(['arc-3', 'ARC-0004', 'ARC-0005'], f.boardId, f.actor, { byUser: true });
    expect(moved.map((t) => [t.id, t.boardId, t.sortOrder, t.rev])).toEqual([
      ['ARC-0003', f.boardId, 1, 2],
      ['ARC-0004', f.boardId, 2, 2],
      ['ARC-0005', f.boardId, 1, 2],
    ]);
    expect(moved[0]).toMatchObject({ column: 'backlog', waiting: false });
    expect((await f.tickets.list({ boardIds: [sourceId] }))).toEqual([]);
    expect((await f.tickets.list({ boardIds: [f.boardId] })).map((t) => t.id)).toEqual(['ARC-0001', 'ARC-0003', 'ARC-0004', 'ARC-0002', 'ARC-0005']);
  });

  it('schreibt einen Verlaufseintrag boardId mit dem Namen des Quell-Boards', async () => {
    const { f, sourceId } = await setup();
    await f.tickets.create(sourceId, { title: 'A' }, f.actor);
    await f.tickets.moveToBoard(['ARC-0001'], f.boardId, f.actor);
    const history = (await f.tickets.get('ARC-0001')).history;
    expect(history).toHaveLength(2);
    expect(history[1]).toMatchObject({ field: 'boardId', from: String(sourceId), to: String(f.boardId), note: 'von Board Kollege geholt', byName: 'claude' });
  });

  it('nimmt nichts, wenn eine ID unbekannt ist (Rollback)', async () => {
    const { f, sourceId } = await setup();
    await f.tickets.create(sourceId, { title: 'A' }, f.actor);
    await expect(f.tickets.moveToBoard(['ARC-0001', 'ARC-0099'], f.boardId, f.actor)).rejects.toMatchObject({ code: 'ticket-not-found' });
    const t = await f.tickets.get('ARC-0001');
    expect(t).toMatchObject({ boardId: sourceId, rev: 1 });
    expect(t.history).toHaveLength(1);
  });

  it('lehnt Tickets ab, die schon auf dem Ziel-Board liegen', async () => {
    const { f, sourceId } = await setup();
    await f.tickets.create(sourceId, { title: 'A' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'B' }, f.actor);
    await expect(f.tickets.moveToBoard(['ARC-0001', 'ARC-0002'], f.boardId, f.actor)).rejects.toMatchObject({ code: 'ticket-already-on-board', message: expect.stringContaining('ARC-0002') });
    expect((await f.tickets.get('ARC-0001')).boardId).toBe(sourceId);
  });

  it('lehnt Tickets aus einem anderen Projekt ab', async () => {
    const { f, sourceId } = await setup();
    await f.projects.create('INC', 'Incidents', f.actor);
    const foreign = await f.boards.create('INC', 'Fremd', f.actor);
    await f.tickets.create(sourceId, { title: 'A' }, f.actor);
    await f.tickets.create(foreign.id, { title: 'Fremd' }, f.actor);
    await expect(f.tickets.moveToBoard(['ARC-0001', 'INC-0001'], f.boardId, f.actor)).rejects.toMatchObject({ code: 'board-other-project' });
    expect((await f.tickets.get('ARC-0001')).boardId).toBe(sourceId);
    expect((await f.tickets.get('INC-0001')).boardId).toBe(foreign.id);
  });

  it('meldet leere Liste und unbekanntes Ziel-Board', async () => {
    const { f, sourceId } = await setup();
    await f.tickets.create(sourceId, { title: 'A' }, f.actor);
    await expect(f.tickets.moveToBoard([], f.boardId, f.actor)).rejects.toMatchObject({ code: 'nothing-changed' });
    await expect(f.tickets.moveToBoard(['ARC-0001'], 999, f.actor)).rejects.toMatchObject({ code: 'board-not-found' });
  });

  it('nimmt doppelt genannte Tickets nur einmal', async () => {
    const { f, sourceId } = await setup();
    await f.tickets.create(sourceId, { title: 'A' }, f.actor);
    const moved = await f.tickets.moveToBoard(['ARC-0001', 'arc-1'], f.boardId, f.actor);
    expect(moved.map((t) => t.id)).toEqual(['ARC-0001']);
    expect((await f.tickets.get('ARC-0001')).rev).toBe(2);
  });

  it('ändert den Änderungskennwert beider Boards', async () => {
    const { f, sourceId } = await setup();
    await f.tickets.create(sourceId, { title: 'A' }, f.actor);
    const before = [await f.db.changeMarker(sourceId), await f.db.changeMarker(f.boardId)];
    await f.tickets.moveToBoard(['ARC-0001'], f.boardId, f.actor);
    expect(await f.db.changeMarker(sourceId)).not.toBe(before[0]);
    expect(await f.db.changeMarker(f.boardId)).not.toBe(before[1]);
  });
});
