import { describe, expect, it } from 'vitest';
import { BoardServices } from '../../../src/board/BoardServices.js';
import { BoardViewService } from '../../../src/board/BoardViewService.js';
import { BoardFixture } from './helpers/BoardFixture.js';

describe('BoardViewService', () => {
  it('liefert alle Spalten in fester Reihenfolge mit sortierten Tickets und Kommentarzahl', async () => {
    const f = await BoardFixture.create();
    const views = new BoardViewService(f.db, f.clock);
    const a = await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Dringend', priority: 'urgent' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Wartet', dependsOn: [a.id] }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Läuft', status: 'in_progress' }, f.actor);
    await f.comments.add(a.id, 'Hallo', f.actor);

    const view = await views.get(f.boardId);

    expect(view.boardId).toBe(f.boardId);
    expect(view.columns.map((c) => [c.id, c.title])).toEqual([
      ['backlog', 'Backlog'], ['waiting', 'Wartet'], ['in_planning', 'In Planung'], ['planned', 'Geplant'],
      ['in_progress', 'In Arbeit'], ['blocked', 'Blockiert'], ['done', 'Erledigt'], ['not_needed', 'Nicht mehr nötig'],
    ]);
    expect(view.columns[0].tickets.map((t) => [t.id, t.commentCount])).toEqual([['ARC-0002', 0], ['ARC-0001', 1]]);
    expect(view.columns[1].tickets).toEqual([expect.objectContaining({ id: 'ARC-0003', waiting: true, dependsOpen: ['ARC-0001'], commentCount: 0 })]);
    expect(view.columns[4].tickets.map((t) => t.id)).toEqual(['ARC-0004']);
    expect(view.columns[2].tickets).toEqual([]);
  });

  it('sendet statt der langen Texte nur, ob sie vorhanden sind', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'Mit Text', description: 'Lang', acceptance: 'Geht' }, f.actor);
    const [card] = (await new BoardViewService(f.db, f.clock).get(f.boardId)).columns.flatMap((c) => c.tickets);
    expect(card).toMatchObject({ hasDescription: true, hasAcceptance: true, hasPlan: false, hasHandbook: false, hasDiagram: false });
    expect(card).not.toHaveProperty('description');
    expect(card).not.toHaveProperty('plan');
  });

  it('zeigt nur Tickets des Boards und meldet ein unbekanntes Board', async () => {
    const f = await BoardFixture.create();
    const other = await f.boards.create('ARC', 'Anderes', f.actor);
    await f.tickets.create(other.id, { title: 'Fremd' }, f.actor);
    const views = new BoardViewService(f.db, f.clock);
    expect((await views.get(f.boardId)).columns.flatMap((c) => c.tickets)).toEqual([]);
    await expect(views.get(999)).rejects.toMatchObject({ code: 'board-not-found' });
  });

  it('ist Teil des Dienst-Bündels', () => {
    expect(new BoardServices(new BoardFixture().db).views).toBeInstanceOf(BoardViewService);
  });
});
