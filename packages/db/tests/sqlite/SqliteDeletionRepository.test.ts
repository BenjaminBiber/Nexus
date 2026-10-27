import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempSqlite, TEST_USER } from './helpers/TempSqlite.js';

describe('SQLite: Löschprotokoll', () => {
  let temp: TempSqlite;

  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  const d1 = { at: '2026-10-13T10:00:00.000Z', byId: TEST_USER.id, byName: 'Tester', ticketId: 'ARC-0007', ticketTitle: 'Ärger mit Umlauten', fromStatus: 'in_progress' as const };
  const d2 = { ...d1, at: '2026-10-13T11:00:00.000Z', ticketId: 'ARC-0008', ticketTitle: 'Zweites', fromStatus: null };

  it('protokolliert und liest in Reihenfolge', async () => {
    await temp.seedUser();

    await temp.db.transaction((u) => u.deletions.insert(d1));
    await temp.db.transaction((u) => u.deletions.insert(d2));

    expect(await temp.db.transaction((u) => u.deletions.list())).toEqual([d1, d2]);
  });

  it('ist leer ohne Einträge', async () => {
    expect(await temp.db.transaction((u) => u.deletions.list())).toEqual([]);
  });

  it('normiert GUID und Datum', async () => {
    await temp.seedUser();

    await temp.db.transaction((u) => u.deletions.insert({ ...d1, byId: TEST_USER.id.toUpperCase(), at: '2026-10-13T10:00:00Z' }));

    const [read] = await temp.db.transaction((u) => u.deletions.list());
    expect(read.byId).toBe(TEST_USER.id);
    expect(read.at).toBe('2026-10-13T10:00:00.000Z');
  });
});
