import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempSqlite, TEST_NOW, TEST_USER } from './helpers/TempSqlite.js';

describe('SqliteProjectRepository', () => {
  let temp: TempSqlite;
  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  const ARC = { key: 'ARC', name: 'Arches', createdAt: TEST_NOW, createdBy: TEST_USER.id };

  it('Projekt anlegen liefert Id', async () => {
    await temp.seedUser();
    const p = await temp.db.transaction((u) => u.projects.insert(ARC));
    expect(p.id).toBeGreaterThan(0);
    expect(p.nextTicketNumber).toBe(1);
    expect(p.nextQuestionNumber).toBe(1);

    expect(await temp.db.transaction((u) => u.projects.byId(p.id))).toEqual(p);
    expect(await temp.db.transaction((u) => u.projects.byKey('ARC'))).toEqual(p);
    expect(await temp.db.transaction((u) => u.projects.byKey('NOPE'))).toBeNull();
    expect(await temp.db.transaction((u) => u.projects.byId(9999))).toBeNull();
  });

  it('doppelter Key führt zu Eindeutigkeitsfehler', async () => {
    await temp.seedUser();
    await temp.db.transaction((u) => u.projects.insert(ARC));

    await expect(temp.db.transaction((u) => u.projects.insert(ARC))).rejects.toMatchObject({ code: 'project-exists' });

    const { rows } = await temp.sql<{ N: number }>('SELECT COUNT(*) AS N FROM Projects');
    expect(Number(rows[0].N)).toBe(1);
  });

  it('zwei Ticketnummern hintereinander sind fortlaufend', async () => {
    await temp.seedUser();
    const p = await temp.db.transaction((u) => u.projects.insert(ARC));

    expect(await temp.db.transaction((u) => u.projects.takeTicketNumber(p.id))).toBe(1);
    expect(await temp.db.transaction((u) => u.projects.takeTicketNumber(p.id))).toBe(2);

    const reloaded = await temp.db.transaction((u) => u.projects.byId(p.id));
    expect(reloaded?.nextTicketNumber).toBe(3);
  });

  it('Fragenummern zählen unabhängig von Ticketnummern', async () => {
    await temp.seedUser();
    const p = await temp.db.transaction((u) => u.projects.insert(ARC));

    expect(await temp.db.transaction((u) => u.projects.takeQuestionNumber(p.id))).toBe(1);
    expect(await temp.db.transaction((u) => u.projects.takeQuestionNumber(p.id))).toBe(2);

    const reloaded = await temp.db.transaction((u) => u.projects.byId(p.id));
    expect(reloaded?.nextTicketNumber).toBe(1);
  });

  it('rename ändert den Namen', async () => {
    await temp.seedUser();
    const p = await temp.db.transaction((u) => u.projects.insert(ARC));
    await temp.db.transaction((u) => u.projects.rename(p.id, 'Neu'));

    const reloaded = await temp.db.transaction((u) => u.projects.byId(p.id));
    expect(reloaded?.name).toBe('Neu');
  });

  it('list sortiert nach Key', async () => {
    await temp.seedUser();
    await temp.db.transaction((u) => u.projects.insert({ ...ARC, key: 'ZED' }));
    await temp.db.transaction((u) => u.projects.insert(ARC));

    const list = await temp.db.transaction((u) => u.projects.list());
    expect(list.map((x) => x.key)).toEqual(['ARC', 'ZED']);
  });
});
