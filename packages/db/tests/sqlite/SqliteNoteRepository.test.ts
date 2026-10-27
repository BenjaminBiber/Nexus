import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { BoardNote } from '@harness/core';
import { TempSqlite, TEST_NOW, TEST_USER } from './helpers/TempSqlite.js';

describe('SQLite: Notizen', () => {
  let temp: TempSqlite;
  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  const note = (over: Partial<Omit<BoardNote, 'id'>> = {}): Omit<BoardNote, 'id'> => ({
    boardId: 1, authorId: TEST_USER.id, authorName: 'Tester', title: 'Cache leert sich', body: 'Befund: Cache leert sich',
    source: null, labels: [], seenCount: 1, seenIn: [], state: 'open', ticketId: null, reason: null, createdAt: TEST_NOW,
    ...over,
  });

  it('legt an und liest', async () => {
    const { boardId } = await temp.seedBoard();
    const n = await temp.db.transaction((u) => u.notes.insert(note({ boardId, labels: ['bug', 'ui'], source: 'cli', body: 'Ärger im Formular' })));

    expect(n.id).toBeGreaterThan(0);
    expect(await temp.db.transaction((u) => u.notes.byId(n.id))).toEqual(n);
    expect(await temp.db.transaction((u) => u.notes.byId(n.id, true))).toEqual(n);
    expect(await temp.db.transaction((u) => u.notes.byId(9999))).toBeNull();
  });

  it('übernimmt eine Notiz (take)', async () => {
    const { boardId } = await temp.seedBoard();
    const n = await temp.db.transaction((u) => u.notes.insert(note({ boardId })));

    await temp.db.transaction((u) => u.notes.update({ ...n, state: 'taken', ticketId: 'ARC-0001' }));

    const updated = await temp.db.transaction((u) => u.notes.byId(n.id));
    expect(updated?.state).toBe('taken');
    expect(updated?.ticketId).toBe('ARC-0001');
  });

  it('verwirft eine Notiz mit Grund (drop)', async () => {
    const { boardId } = await temp.seedBoard();
    const n = await temp.db.transaction((u) => u.notes.insert(note({ boardId })));

    await temp.db.transaction((u) => u.notes.update({ ...n, state: 'dropped', reason: 'doppelt' }));

    const updated = await temp.db.transaction((u) => u.notes.byId(n.id));
    expect(updated?.state).toBe('dropped');
    expect(updated?.reason).toBe('doppelt');
  });

  it('filtert nach Board, Status und Quelle', async () => {
    const { projectId, boardId } = await temp.seedBoard();
    const b2 = await temp.db.transaction((u) => u.boards.insert({ projectId, ownerId: TEST_USER.id, name: 'Anderes', repoPath: null, createdAt: TEST_NOW }));

    const open1 = await temp.db.transaction((u) => u.notes.insert(note({ boardId, source: 'cli' })));
    const taken1 = await temp.db.transaction((u) => u.notes.insert(note({ boardId, state: 'taken', ticketId: 'ARC-0001' })));
    await temp.db.transaction((u) => u.notes.insert(note({ boardId: b2.id })));

    expect(await temp.db.transaction((u) => u.notes.list({ boardId }))).toEqual([open1, taken1]);
    expect(await temp.db.transaction((u) => u.notes.list({ boardId, states: ['open'] }))).toEqual([open1]);
    expect(await temp.db.transaction((u) => u.notes.list({ boardId, states: [] }))).toEqual([]);
    expect(await temp.db.transaction((u) => u.notes.list({ boardId, source: 'cli' }))).toEqual([open1]);
    expect((await temp.db.transaction((u) => u.notes.list({ boardId: b2.id }))).map((x) => x.id)).not.toContain(open1.id);
  });

  it('speichert Titel und Zähler; ältere Zeilen ohne diese Spalten bekommen leeren Titel und 1', async () => {
    const { boardId } = await temp.seedBoard();
    const n = await temp.db.transaction((u) => u.notes.insert(note({ boardId })));
    await temp.db.transaction((u) => u.notes.update({ ...n, title: 'Anderer Titel', seenCount: 4, seenIn: ['ARC-0002', 'ARC-0003'] }));
    expect(await temp.db.transaction((u) => u.notes.byId(n.id))).toMatchObject({ title: 'Anderer Titel', seenCount: 4, seenIn: ['ARC-0002', 'ARC-0003'] });

    const { rows } = await temp.sql<{ Id: number }>(
      "INSERT INTO Notes (BoardId, AuthorId, AuthorName, Body, State, CreatedAt) VALUES (@b, @u, 'Alt', 'Alter Befund', 'open', @now) RETURNING Id",
      { b: boardId, u: TEST_USER.id, now: TEST_NOW },
    );
    expect(await temp.db.transaction((u) => u.notes.byId(rows[0].Id))).toMatchObject({ title: '', body: 'Alter Befund', seenCount: 1, seenIn: [] });
  });

  it('leere Labels und GUID kleingeschrieben', async () => {
    const { boardId } = await temp.seedBoard();

    const n = await temp.db.transaction((u) => u.notes.insert(note({ boardId, authorId: TEST_USER.id.toUpperCase(), labels: [] })));

    expect(n.labels).toEqual([]);
    expect(n.authorId).toBe(TEST_USER.id);
  });
});
