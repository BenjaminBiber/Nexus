import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempSqlite, TEST_NOW, TEST_USER } from './helpers/TempSqlite.js';

describe('SqliteBoardRepository', () => {
  let temp: TempSqlite;
  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  it('anlegen und lesen', async () => {
    const projectId = await temp.seedProject();
    const SPRINT = { projectId, ownerId: TEST_USER.id, name: 'Sprint', repoPath: null, createdAt: TEST_NOW };

    const b = await temp.db.transaction((u) => u.boards.insert(SPRINT));
    expect(b.id).toBeGreaterThan(0);
    expect(await temp.db.transaction((u) => u.boards.byId(b.id))).toEqual(b);
    expect(await temp.db.transaction((u) => u.boards.byId(9999))).toBeNull();
  });

  it('auflisten mit Filtern', async () => {
    const projectId = await temp.seedProject();
    const OTHER = { id: '22222222-2222-2222-2222-222222222222', name: 'Zweiter' };
    await temp.seedUser(OTHER);
    const SPRINT = { projectId, ownerId: TEST_USER.id, name: 'Sprint', repoPath: null, createdAt: TEST_NOW };

    const b1 = await temp.db.transaction((u) => u.boards.insert(SPRINT));
    const b2 = await temp.db.transaction((u) => u.boards.insert({ ...SPRINT, ownerId: OTHER.id, repoPath: 'C:/repo' }));
    const p2 = await temp.seedProject('ZED');
    const b3 = await temp.db.transaction((u) => u.boards.insert({ ...SPRINT, projectId: p2 }));

    expect((await temp.db.transaction((u) => u.boards.list())).map((b) => b.id)).toEqual([b1.id, b2.id, b3.id]);
    expect(await temp.db.transaction((u) => u.boards.list({ projectId }))).toEqual([b1, b2]);
    expect(await temp.db.transaction((u) => u.boards.list({ ownerId: OTHER.id }))).toEqual([b2]);
    expect(await temp.db.transaction((u) => u.boards.list({ ownerId: OTHER.id.toUpperCase() }))).toEqual([b2]);
    expect(await temp.db.transaction((u) => u.boards.list({ projectId, ownerId: TEST_USER.id }))).toEqual([b1]);
  });

  it('ändern wie im MSSQL-Repository', async () => {
    const projectId = await temp.seedProject();
    const SPRINT = { projectId, ownerId: TEST_USER.id, name: 'Sprint', repoPath: null, createdAt: TEST_NOW };
    const b1 = await temp.db.transaction((u) => u.boards.insert(SPRINT));

    const m0 = await temp.db.changeMarker(b1.id);
    await temp.db.transaction((u) => u.boards.update({ ...b1, name: 'Neu', repoPath: 'D:/x' }));
    const updated = await temp.db.transaction((u) => u.boards.byId(b1.id));
    expect(updated?.name).toBe('Neu');
    expect(updated?.repoPath).toBe('D:/x');
    expect(updated?.id).toBe(b1.id);
    expect(updated?.projectId).toBe(b1.projectId);
    expect(updated?.ownerId).toBe(b1.ownerId);
    expect(updated?.createdAt).toBe(b1.createdAt);
    expect(await temp.db.changeMarker(b1.id)).not.toBe(m0);
  });

  it('löschen entfernt Notizen', async () => {
    const projectId = await temp.seedProject();
    const SPRINT = { projectId, ownerId: TEST_USER.id, name: 'Sprint', repoPath: null, createdAt: TEST_NOW };
    const b1 = await temp.db.transaction((u) => u.boards.insert(SPRINT));

    await temp.sql(
      "INSERT INTO Notes (BoardId, AuthorId, AuthorName, Body, State, CreatedAt) VALUES (@b, @u, 'Test', 'Notiz', 'open', @now)",
      { b: b1.id, u: TEST_USER.id, now: TEST_NOW },
    );

    await temp.db.transaction((u) => u.boards.delete(b1.id));
    expect(await temp.db.transaction((u) => u.boards.byId(b1.id))).toBeNull();
    expect((await temp.sql<{ N: number }>('SELECT COUNT(*) AS N FROM Notes')).rows[0].N).toBe(0);
  });
});
