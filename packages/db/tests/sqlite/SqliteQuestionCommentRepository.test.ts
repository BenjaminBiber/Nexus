import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempSqlite, TEST_NOW, TEST_USER } from './helpers/TempSqlite.js';

describe('SQLite: Kommentare an Fragen', () => {
  let temp: TempSqlite;

  beforeEach(async () => {
    temp = await TempSqlite.create();
    const projectId = await temp.seedProject();
    await temp.sql(
      `INSERT INTO Questions (Id, ProjectId, Number, Title, State, OriginKind, OriginName, CreatedAt, CreatedById, UpdatedAt, Rev)
       VALUES ('ARC-F0001', @projectId, 1, 'Welcher Speicher?', 'open', 'session', 'claude', @now, @userId, @now, 1)`,
      { projectId, now: TEST_NOW, userId: TEST_USER.id },
    );
    await temp.sql(
      `INSERT INTO Questions (Id, ProjectId, Number, Title, State, OriginKind, OriginName, CreatedAt, CreatedById, UpdatedAt, Rev)
       VALUES ('ARC-F0002', @projectId, 2, 'Zweite Frage', 'open', 'session', 'claude', @now, @userId, @now, 1)`,
      { projectId, now: TEST_NOW, userId: TEST_USER.id },
    );
  });
  afterEach(async () => { await temp.dispose(); });

  it('legt Kommentare an und liest sie', async () => {
    const c1 = await temp.db.transaction((u) => u.questionComments.insert({
      questionId: 'ARC-F0001', authorId: TEST_USER.id, authorName: 'Florian', authorKind: 'human', body: 'Kommentar zur Lizenz äöü', createdAt: TEST_NOW,
    }));
    const c2 = await temp.db.transaction((u) => u.questionComments.insert({
      questionId: 'ARC-F0001', authorId: TEST_USER.id, authorName: 'Florian', authorKind: 'session', body: 'Noch einer', createdAt: '2026-10-13T10:01:00.000Z',
    }));

    expect(c1.id).toBeGreaterThan(0);
    expect(c2.id).toBeGreaterThan(c1.id);
    expect(await temp.db.transaction((u) => u.questionComments.list('ARC-F0001'))).toEqual([c1, c2]);
  });

  it('listet nur Kommentare der eigenen Frage', async () => {
    await temp.db.transaction((u) => u.questionComments.insert({
      questionId: 'ARC-F0002', authorId: TEST_USER.id, authorName: 'Florian', authorKind: 'human', body: 'Gehört zur zweiten Frage', createdAt: TEST_NOW,
    }));

    expect(await temp.db.transaction((u) => u.questionComments.list('ARC-F0001'))).toEqual([]);
    expect(await temp.db.transaction((u) => u.questionComments.list('ARC-F0099'))).toEqual([]);
  });

  it('normiert GUID und Datum', async () => {
    await temp.db.transaction((u) => u.questionComments.insert({
      questionId: 'ARC-F0001', authorId: TEST_USER.id.toUpperCase(), authorName: 'Florian', authorKind: 'human', body: 'Groß geschrieben', createdAt: '2026-10-13T10:00:00Z',
    }));

    const [read] = await temp.db.transaction((u) => u.questionComments.list('ARC-F0001'));
    expect(read.authorId).toBe(TEST_USER.id);
    expect(read.createdAt).toBe('2026-10-13T10:00:00.000Z');
  });
});
