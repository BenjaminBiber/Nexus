import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempSqlite, TEST_USER } from './helpers/TempSqlite.js';

describe('SqliteUserRepository', () => {
  let temp: TempSqlite;
  beforeEach(async () => { temp = await TempSqlite.create(); });
  afterEach(async () => { await temp.dispose(); });

  it('upsert legt an', async () => {
    await temp.db.transaction((u) => u.users.upsert({ id: TEST_USER.id, name: 'Anna' }));
    const found = await temp.db.transaction((u) => u.users.byId(TEST_USER.id));
    expect(found).toEqual({ id: TEST_USER.id, name: 'Anna' });
  });

  it('upsert aktualisiert den Namen ohne Duplikat', async () => {
    await temp.db.transaction((u) => u.users.upsert({ id: TEST_USER.id, name: 'Anna' }));
    await temp.db.transaction((u) => u.users.upsert({ id: TEST_USER.id, name: 'Berta' }));
    const found = await temp.db.transaction((u) => u.users.byId(TEST_USER.id));
    expect(found?.name).toBe('Berta');
    const { rows } = await temp.sql<{ N: number }>('SELECT COUNT(*) AS N FROM Users');
    expect(Number(rows[0].N)).toBe(1);
  });

  it('speichert GUIDs klein', async () => {
    const upper = 'AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA';
    const lower = upper.toLowerCase();
    await temp.db.transaction((u) => u.users.upsert({ id: upper, name: 'Groß' }));
    const { rows } = await temp.sql<{ Id: string }>('SELECT Id FROM Users');
    expect(rows[0].Id).toBe(lower);
    const byUpper = await temp.db.transaction((u) => u.users.byId(upper));
    const byLower = await temp.db.transaction((u) => u.users.byId(lower));
    expect(byUpper?.id).toBe(lower);
    expect(byLower?.id).toBe(lower);
  });

  it('list sortiert nach Name ohne Groß/Klein', async () => {
    await temp.db.transaction((u) => u.users.upsert({ id: '22222222-2222-2222-2222-222222222222', name: 'berta' }));
    await temp.db.transaction((u) => u.users.upsert({ id: '11111111-1111-1111-1111-111111111111', name: 'Anna' }));
    await temp.db.transaction((u) => u.users.upsert({ id: '33333333-3333-3333-3333-333333333333', name: 'Carl' }));
    const names = (await temp.db.transaction((u) => u.users.list())).map((u) => u.name);
    expect(names).toEqual(['Anna', 'berta', 'Carl']);
  });

  it('byId unbekannt liefert null', async () => {
    const found = await temp.db.transaction((u) => u.users.byId('99999999-9999-9999-9999-999999999999'));
    expect(found).toBeNull();
  });
});
