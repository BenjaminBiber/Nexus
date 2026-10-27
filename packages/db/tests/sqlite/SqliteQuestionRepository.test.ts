import type { BoardQuestion, BoardQuestionFilter } from '@harness/core';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TEST_NOW, TEST_USER, TempSqlite } from './helpers/TempSqlite.js';

const USER = TEST_USER.id;
const NOW = TEST_NOW;
const LATER = '2026-10-13T11:00:00.000Z';

describe('SQLite: Fragen', () => {
  let temp: TempSqlite;
  let projectId: number;
  let otherProjectId: number;

  beforeEach(async () => {
    temp = await TempSqlite.create();
    const board = await temp.seedBoard();
    projectId = board.projectId;
    await temp.seedTicket({ id: 'ARC-0001', projectId, boardId: board.boardId, number: 1, title: 'Login' });
    await temp.seedTicket({ id: 'ARC-0002', projectId, boardId: board.boardId, number: 2, title: 'Logout' });
    otherProjectId = await temp.seedProject('OTH');
  });
  afterEach(async () => { await temp.dispose(); });

  // Grundgerüst einer Frage; über `over` je Fall angepasst
  const question = (over: Partial<BoardQuestion> = {}): BoardQuestion => ({
    id: 'ARC-F0001', projectId, number: 1, title: 'Welcher Speicher?', description: '', state: 'open', occasion: null,
    originKind: 'session', originName: 'claude', originSession: null,
    tags: [], ticketIds: [], dependsOn: [], decision: null,
    createdAt: NOW, createdById: USER, updatedAt: NOW, rev: 1, ...over,
  });

  const insert = (q: BoardQuestion) => temp.db.transaction((u) => u.questions.insert(q));
  const update = (q: BoardQuestion) => temp.db.transaction((u) => u.questions.update(q));
  const byId = (id: string) => temp.db.transaction((u) => u.questions.byId(id));

  it('legt eine Frage mit Tags, Tickets und Abhängigkeiten an', async () => {
    const q = question({ tags: ['db', 'api'], ticketIds: ['ARC-0002', 'ARC-0001'], dependsOn: ['ARC-F0002', 'ARC-0001'] });

    await insert(q);

    expect(await byId('ARC-F0001')).toEqual({ ...q, ticketIds: ['ARC-0001', 'ARC-0002'] });
    expect(await temp.db.transaction((u) => u.questions.byId('ARC-F0001', true))).toEqual({ ...q, ticketIds: ['ARC-0001', 'ARC-0002'] });
    expect(await byId('ARC-F0099')).toBeNull();
  });

  it('beantwortet eine Frage und ersetzt die Tags', async () => {
    await insert(question({ tags: ['db', 'api'] }));
    const answered = question({
      rev: 2, state: 'answered', tags: ['db'], updatedAt: LATER,
      decision: { outcome: 'answered', option: 'O1', text: 'SQL Server', reason: 'Lizenz', byId: USER, byName: 'Florian', at: LATER },
    });

    await update(answered);

    expect(await byId('ARC-F0001')).toEqual(answered);
  });

  it('verwirft eine Frage und öffnet sie wieder', async () => {
    await insert(question());
    await update(question({
      rev: 2, state: 'discarded', updatedAt: LATER,
      decision: { outcome: 'discarded', option: null, text: null, reason: 'überholt', byId: USER, byName: 'Florian', at: LATER },
    }));

    const discarded = await byId('ARC-F0001');
    expect(discarded?.decision?.outcome).toBe('discarded');
    expect(discarded?.decision?.text).toBeNull();

    await update(question({ rev: 3, state: 'open', decision: null, updatedAt: LATER }));

    const reopened = await byId('ARC-F0001');
    expect(reopened?.decision).toBeNull();
    expect(reopened?.rev).toBe(3);
  });

  it('meldet einen Rev-Konflikt und ändert nichts', async () => {
    await insert(question());
    const stale = question({ rev: 3, title: 'Neu' });

    await expect(update(stale)).rejects.toMatchObject({ code: 'conflict' });

    const unchanged = await byId('ARC-F0001');
    expect(unchanged?.rev).toBe(1);
    expect(unchanged?.title).toBe('Welcher Speicher?');
  });

  it('filtert Fragen', async () => {
    await insert(question({ id: 'ARC-F0001', number: 1, ticketIds: ['ARC-0001'] }));
    await insert(question({
      id: 'ARC-F0002', number: 2, state: 'answered',
      decision: { outcome: 'answered', option: null, text: 'ja', reason: '', byId: USER, byName: 'Florian', at: LATER },
    }));
    await insert(question({ id: 'OTH-F0001', projectId: otherProjectId, number: 1 }));

    const ids = async (filter?: BoardQuestionFilter) => (await temp.db.transaction((u) => u.questions.list(filter))).map((q) => q.id);

    expect(await ids()).toEqual(['ARC-F0001', 'ARC-F0002', 'OTH-F0001']);
    expect(await ids({ projectId })).toEqual(['ARC-F0001', 'ARC-F0002']);
    expect(await ids({ states: ['open'] })).toEqual(['ARC-F0001', 'OTH-F0001']);
    expect(await ids({ ticketIds: ['ARC-0001'] })).toEqual(['ARC-F0001']);
    expect(await ids({ ids: [] })).toEqual([]);
    expect(await ids({ states: [] })).toEqual([]);
    expect(await ids({ ticketIds: [] })).toEqual([]);
    expect(await ids({ projectId, ids: ['ARC-F0002', 'OTH-F0001'] })).toEqual(['ARC-F0002']);
  });

  it('findet und löst Bezüge auf ein Ticket', async () => {
    await insert(question({ id: 'ARC-F0001', number: 1, ticketIds: ['ARC-0001', 'ARC-0002'] }));
    await insert(question({ id: 'ARC-F0002', number: 2, dependsOn: ['ARC-0001', 'ARC-0002'] }));

    expect(await temp.db.transaction((u) => u.questions.idsReferencingTicket('ARC-0001'))).toEqual(['ARC-F0001', 'ARC-F0002']);
    expect(await temp.db.transaction((u) => u.questions.removeTicket('ARC-0001'))).toEqual(['ARC-F0001', 'ARC-F0002']);

    expect(await temp.db.transaction((u) => u.questions.idsReferencingTicket('ARC-0001'))).toEqual([]);
    const first = await byId('ARC-F0001');
    expect(first?.ticketIds).toEqual(['ARC-0002']);
    expect(first?.rev).toBe(1);
    const second = await byId('ARC-F0002');
    expect(second?.dependsOn).toEqual(['ARC-0002']);
    expect(second?.rev).toBe(1);
  });

  it('speichert GUIDs kleingeschrieben', async () => {
    await insert(question({ id: 'ARC-F0003', number: 3, createdById: USER.toUpperCase() }));

    expect((await byId('ARC-F0003'))?.createdById).toBe(USER);
  });
});
