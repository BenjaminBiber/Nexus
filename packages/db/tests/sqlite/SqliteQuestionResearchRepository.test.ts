import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { BoardQuestionOption } from '@harness/core';
import { TempSqlite, TEST_NOW, TEST_USER } from './helpers/TempSqlite.js';

describe('SQLite: Recherchen', () => {
  let temp: TempSqlite;
  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  // Legt eine Frage mit Mindestfeldern an (Rest über Spalten-Defaults)
  const seedQuestion = (id: string, projectId: number, number: number) => temp.sql(
    `INSERT INTO Questions (Id, ProjectId, Number, Title, State, OriginKind, OriginName, CreatedAt, CreatedById, UpdatedAt, Rev)
     VALUES (@id, @projectId, @number, @title, 'open', 'session', 'claude', @now, @userId, @now, 1)`,
    { id, projectId, number, title: `Frage ${number}`, now: TEST_NOW, userId: TEST_USER.id },
  );

  const option = (id: string, recommended = false): BoardQuestionOption => ({
    id, title: `Variante ${id}`, text: 'Text äöü', pros: ['schnell'], cons: [], recommended,
  });

  it('speichert mit Optionen und liest in Optionsreihenfolge', async () => {
    const projectId = await temp.seedProject();
    await seedQuestion('ARC-F0001', projectId, 1);

    const r1 = await temp.db.transaction((u) => u.questionResearch.insert({
      questionId: 'ARC-F0001', at: TEST_NOW, byId: TEST_USER.id, byName: 'claude', kind: 'session', context: 'Kontext',
      options: [option('O1', true), option('O2')],
    }));
    const r2 = await temp.db.transaction((u) => u.questionResearch.insert({
      questionId: 'ARC-F0001', at: '2026-10-13T11:00:00.000Z', byId: TEST_USER.id, byName: 'claude', kind: 'subagent', context: '',
      options: [option('O10'), option('O3')],
    }));
    expect(r2.id).toBeGreaterThan(r1.id);

    const list = await temp.db.transaction((u) => u.questionResearch.list('ARC-F0001'));
    expect(list).toEqual([r1, { ...r2, options: [option('O3'), option('O10')] }]);
    expect(list[0].options[0].recommended).toBe(true);
    expect(list[0].options[1].recommended).toBe(false);
    expect(Array.isArray(list[0].options[0].pros)).toBe(true);
  });

  it('liefert eine leere Liste ohne Recherchen', async () => {
    const projectId = await temp.seedProject();
    await seedQuestion('ARC-F0002', projectId, 2);

    await expect(temp.db.transaction((u) => u.questionResearch.list('ARC-F0002'))).resolves.toEqual([]);
  });

  it('fasst Zahl der Recherchen und Optionen der neuesten je Frage zusammen', async () => {
    const projectId = await temp.seedProject();
    await seedQuestion('ARC-F0001', projectId, 1);
    await seedQuestion('ARC-F0002', projectId, 2);

    await temp.db.transaction((u) => u.questionResearch.insert({
      questionId: 'ARC-F0001', at: TEST_NOW, byId: TEST_USER.id, byName: 'claude', kind: 'session', context: 'Kontext',
      options: [option('O1', true), option('O2')],
    }));
    await temp.db.transaction((u) => u.questionResearch.insert({
      questionId: 'ARC-F0001', at: '2026-10-13T11:00:00.000Z', byId: TEST_USER.id, byName: 'claude', kind: 'subagent', context: '',
      options: [option('O10'), option('O3')],
    }));
    await temp.db.transaction((u) => u.questionResearch.insert({
      questionId: 'ARC-F0002', at: TEST_NOW, byId: TEST_USER.id, byName: 'claude', kind: 'session', context: '', options: [],
    }));

    await expect(temp.db.transaction((u) => u.questionResearch.summaries(['ARC-F0001', 'ARC-F0002', 'ARC-F0099']))).resolves.toEqual({
      'ARC-F0001': { count: 2, optionCount: 2 },
      'ARC-F0002': { count: 1, optionCount: 0 },
    });
    await expect(temp.db.transaction((u) => u.questionResearch.summaries([]))).resolves.toEqual({});
  });

  it('normiert GUID und Datum beim Speichern', async () => {
    const projectId = await temp.seedProject();
    await seedQuestion('ARC-F0001', projectId, 1);

    await temp.db.transaction((u) => u.questionResearch.insert({
      questionId: 'ARC-F0001', at: '2026-10-13T10:00:00Z', byId: TEST_USER.id.toUpperCase(), byName: 'claude', kind: 'session',
      context: '', options: [],
    }));

    const [stored] = await temp.db.transaction((u) => u.questionResearch.list('ARC-F0001'));
    expect(stored.byId).toBe(TEST_USER.id);
    expect(stored.at).toBe('2026-10-13T10:00:00.000Z');
  });
});
