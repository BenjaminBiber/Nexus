import { afterEach, beforeEach, expect, it } from 'vitest';
import { BoardServices, type BoardActor } from '@harness/core';
import { MssqlBoardDatabase } from '../src/MssqlBoardDatabase.js';
import { TestDb } from './helpers/TestDb.js';

const config = TestDb.config();
const USER = { id: '11111111-1111-1111-1111-111111111111', name: 'Florian' };
const ACTOR: BoardActor = { userId: USER.id, name: 'Florian' };

// Tickets von einem anderen Board holen auf SQL Server: alles oder nichts, Kennwert beider Boards ändert sich
TestDb.describe('SQL Server: Tickets von anderem Board holen', () => {
  let db: MssqlBoardDatabase;
  let services: BoardServices;

  beforeEach(async () => {
    await TestDb.reset();
    db = await MssqlBoardDatabase.connect(config!);
    services = new BoardServices(db);
    await db.transaction((u) => u.users.upsert(USER));
    await services.projects.create('ARC', 'Arches', ACTOR);
  });
  afterEach(() => db.close());

  it('holt Tickets ans Ende der Spalte und rollt bei unbekannter ID alles zurück', async () => {
    const target = await services.boards.create('ARC', 'Sprint', ACTOR);
    const source = await services.boards.create('ARC', 'Kollege', ACTOR);
    await services.tickets.create(target.id, { title: 'Schon da' }, ACTOR);
    await services.tickets.create(source.id, { title: 'A' }, ACTOR);
    await services.tickets.create(source.id, { title: 'B' }, ACTOR);

    await expect(services.tickets.moveToBoard(['ARC-0002', 'ARC-0099'], target.id, ACTOR)).rejects.toMatchObject({ code: 'ticket-not-found' });
    expect((await services.tickets.get('ARC-0002')).boardId).toBe(source.id);

    const markers = [await db.changeMarker(source.id), await db.changeMarker(target.id)];
    const moved = await services.tickets.moveToBoard(['ARC-0002', 'ARC-0003'], target.id, ACTOR, { byUser: true });
    expect(moved.map((t) => [t.id, t.boardId, t.sortOrder, t.rev])).toEqual([['ARC-0002', target.id, 1, 2], ['ARC-0003', target.id, 2, 2]]);
    expect((await services.tickets.get('ARC-0002')).history.at(-1)).toMatchObject({ field: 'boardId', note: 'von Board Kollege geholt' });
    expect(await db.changeMarker(source.id)).not.toBe(markers[0]);
    expect(await db.changeMarker(target.id)).not.toBe(markers[1]);
  });
});
