import type { BoardQuestionHistoryEntry } from '@harness/core';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempSqlite, TEST_NOW, TEST_USER } from './helpers/TempSqlite.js';

describe('SQLite: Verlauf der Fragen', () => {
  let temp: TempSqlite;
  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  // Legt eine Frage mit Mindestfeldern an
  async function seedQuestion(id: string, projectId: number, number: number): Promise<void> {
    await temp.sql(
      `INSERT INTO Questions (Id, ProjectId, Number, Title, State, OriginKind, OriginName, CreatedAt, CreatedById, UpdatedAt, Rev)
       VALUES (@id, @projectId, @number, 'Frage', 'open', 'session', 'claude', @now, @userId, @now, 1)`,
      { id, projectId, number, now: TEST_NOW, userId: TEST_USER.id },
    );
  }

  const h1: BoardQuestionHistoryEntry = {
    questionId: 'ARC-F0001', at: TEST_NOW, byId: TEST_USER.id, byName: 'claude', byKind: 'session', type: 'created', text: 'Frage angelegt',
  };

  it('schreibt und liest in Reihenfolge', async () => {
    const projectId = await temp.seedProject();
    await seedQuestion('ARC-F0001', projectId, 1);

    const h2: BoardQuestionHistoryEntry = {
      ...h1, at: '2026-10-13T10:05:00.000Z', byName: 'Florian', byKind: 'human', type: 'decided', text: 'Beantwortet: SQL Server – Lizenz äöü',
    };
    await temp.db.transaction((u) => u.questionHistory.insert([h1, h2]));

    const h3: BoardQuestionHistoryEntry = {
      ...h1, at: '2026-10-13T10:10:00.000Z', byKind: null, type: 'ticket-deleted', text: 'ARC-0001 gelöscht',
    };
    await temp.db.transaction((u) => u.questionHistory.insert([h3]));

    await expect(temp.db.transaction((u) => u.questionHistory.list('ARC-F0001'))).resolves.toEqual([h1, h2, h3]);
  });

  it('liefert nur Einträge der eigenen Frage', async () => {
    const projectId = await temp.seedProject();
    await seedQuestion('ARC-F0001', projectId, 1);
    await seedQuestion('ARC-F0002', projectId, 2);

    await temp.db.transaction((u) => u.questionHistory.insert([h1]));
    await temp.db.transaction((u) => u.questionHistory.insert([{ ...h1, questionId: 'ARC-F0002', text: 'Andere Frage' }]));
    await temp.db.transaction((u) => u.questionHistory.insert([]));

    await expect(temp.db.transaction((u) => u.questionHistory.list('ARC-F0001'))).resolves.toEqual([h1]);
  });

  it('normiert GUID und Datum', async () => {
    const projectId = await temp.seedProject();
    await seedQuestion('ARC-F0001', projectId, 1);

    await temp.db.transaction((u) => u.questionHistory.insert([{ ...h1, byId: TEST_USER.id.toUpperCase(), at: '2026-10-13T10:00:00Z' }]));

    const [entry] = await temp.db.transaction((u) => u.questionHistory.list('ARC-F0001'));
    expect(entry.byId).toBe(TEST_USER.id);
    expect(entry.at).toBe('2026-10-13T10:00:00.000Z');
  });
});
