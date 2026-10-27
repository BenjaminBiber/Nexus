import { describe, expect, it } from 'vitest';
import { BoardServices } from '../../../src/board/BoardServices.js';
import { DependencyGraphService } from '../../../src/board/DependencyGraphService.js';
import { BoardFixture } from './helpers/BoardFixture.js';

describe('DependencyGraphService', () => {
  // Board „Sprint“ mit A ← B, Board „Zweites“ mit C ← B; B und C gehören zur Spec 001-login
  async function setup() {
    const f = await BoardFixture.create();
    const second = (await f.boards.create('ARC', 'Zweites', f.actor)).id;
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'B', dependsOn: ['ARC-0001'], spec: '001-login' }, f.actor);
    await f.tickets.create(second, { title: 'C', dependsOn: ['ARC-0002'], spec: '001-login' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Allein' }, f.actor);
    return { f, second, service: new DependencyGraphService(f.db, f.clock) };
  }

  it('liefert den Graph eines Tickets über Boards mit Board-Namen', async () => {
    const { service, second } = await setup();
    const g = await service.build({ kind: 'ticket', id: 'arc-2' });
    expect(g.scope).toEqual({ kind: 'ticket', id: 'ARC-0002' });
    expect(g.nodes.map((n) => [n.id, n.boardName])).toEqual([['ARC-0001', 'Sprint'], ['ARC-0002', 'Sprint'], ['ARC-0003', 'Zweites']]);
    expect(g.nodes[2].boardId).toBe(second);
    expect(g.criticalPath).toEqual(['ARC-0001', 'ARC-0002']);
  });

  it('löst den Projekt-Key der Spec auf und zählt kantenlose Board-Tickets', async () => {
    const { f, service } = await setup();
    const spec = await service.build({ kind: 'spec', project: 'arc', spec: '001-login' });
    expect(spec.scope).toEqual({ kind: 'spec', project: 'ARC', spec: '001-login' });
    expect(spec.nodes.map((n) => [n.id, n.external])).toEqual([['ARC-0001', true], ['ARC-0002', false], ['ARC-0003', false]]);
    const board = await service.build({ kind: 'board', boardId: f.boardId });
    expect(board.hiddenWithoutEdges).toBe(1);
    expect(board.nodes.map((n) => n.id)).toEqual(['ARC-0001', 'ARC-0002', 'ARC-0003']);
  });

  it('meldet unbekannte Tickets, Projekte und Boards', async () => {
    const { service } = await setup();
    await expect(service.build({ kind: 'ticket', id: 'ARC-0099' })).rejects.toMatchObject({ code: 'ticket-not-found' });
    await expect(service.build({ kind: 'spec', project: 'XYZ', spec: 'x' })).rejects.toMatchObject({ code: 'project-not-found' });
    await expect(service.build({ kind: 'board', boardId: 999 })).rejects.toMatchObject({ code: 'board-not-found' });
  });

  it('zählt den Fortschritt der Specs eines Boards projektweit', async () => {
    const { f, service, second } = await setup();
    const expected = [{ spec: '001-login', total: 2, done: 0, blocked: 0, waiting: 2, inProgress: 0, open: 0, notNeeded: 0 }];
    expect(await service.forBoard(f.boardId)).toEqual(expected);
    expect(await service.forBoard(second)).toEqual(expected);
    expect(await service.forProject(1)).toEqual(expected);
  });

  it('ist Teil des Dienst-Bündels', () => {
    expect(new BoardServices(new BoardFixture().db).graph).toBeInstanceOf(DependencyGraphService);
  });
});
