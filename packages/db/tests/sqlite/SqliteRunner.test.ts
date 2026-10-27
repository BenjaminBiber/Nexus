import type { DatabaseSync } from 'node:sqlite';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { NodeSqlite } from '../../src/sqlite/NodeSqlite.js';
import { SqliteRunner } from '../../src/sqlite/SqliteRunner.js';

const { DatabaseSync: Database } = NodeSqlite.load();

describe('SqliteRunner', () => {
  let db: DatabaseSync;
  let sql: SqliteRunner;

  beforeEach(async () => {
    db = new Database(':memory:');
    sql = new SqliteRunner(db);
    await sql.query('CREATE TABLE T (Id INTEGER PRIMARY KEY, Name TEXT NULL, Flag INTEGER NULL, At TEXT NULL, UserId TEXT NULL)');
  });
  afterEach(() => db.close());

  it('Datum hin und zurück', async () => {
    const at = new Date('2026-10-21T07:45:42.579Z');
    await sql.query('INSERT INTO T (At) VALUES (@at)', { at });
    const { rows } = await sql.query<{ At: string }>('SELECT At FROM T');
    expect(rows[0].At).toBe('2026-10-21T07:45:42.579Z');
    expect(SqliteRunner.iso(rows[0].At)).toBe(at.toISOString());
    expect(SqliteRunner.date(rows[0].At)!.getTime()).toBe(at.getTime());
    expect(SqliteRunner.iso('2026-10-21T07:45:42Z')).toBe('2026-10-21T07:45:42.000Z');
  });

  it('bool', async () => {
    await sql.query('INSERT INTO T (Flag) VALUES (@flag)', { flag: true });
    await sql.query('INSERT INTO T (Flag) VALUES (@flag)', { flag: false });
    const { rows } = await sql.query<{ Flag: number }>('SELECT Flag FROM T ORDER BY Id');
    expect(rows[0].Flag).toBe(1);
    expect(rows[1].Flag).toBe(0);
    expect(SqliteRunner.bool(1)).toBe(true);
    expect(SqliteRunner.bool(0)).toBe(false);
    expect(SqliteRunner.bool(null)).toBe(false);
    expect(SqliteRunner.bool(true)).toBe(true);
  });

  it('GUID', async () => {
    await sql.query('INSERT INTO T (UserId) VALUES (@userId)', { userId: 'ABCDEF00-0000-0000-0000-00000000000A' });
    const { rows } = await sql.query<{ UserId: string }>('SELECT UserId FROM T');
    expect(SqliteRunner.guid(rows[0].UserId)).toBe('abcdef00-0000-0000-0000-00000000000a');
  });

  it('NULL', async () => {
    await sql.query('INSERT INTO T (Name, At) VALUES (@name, @at)', { name: null, at: undefined });
    const { rows } = await sql.query<{ Name: string | null; At: string | null }>('SELECT Name, At FROM T');
    expect(rows[0].Name).toBeNull();
    expect(rows[0].At).toBeNull();
    expect(SqliteRunner.iso(null)).toBeNull();
    expect(SqliteRunner.date(null)).toBeNull();
  });

  it('Umlaute', async () => {
    await sql.query('INSERT INTO T (Name) VALUES (@name)', { name: 'Ä ö ß' });
    const { rows } = await sql.query<{ Name: string }>('SELECT Name FROM T');
    expect(rows[0].Name).toBe('Ä ö ß');
  });

  it('benannte Parameter mit @', async () => {
    const { rows } = await sql.query<{ x: number; y: number; z: null }>('SELECT @a AS x, @a AS y, @fehlt AS z', { a: 5, unbenutzt: 1 });
    expect(rows).toEqual([{ x: 5, y: 5, z: null }]);

    const { rows: rows2 } = await sql.query<{ v: string }>("SELECT '@a' AS v -- @b", {});
    expect(rows2).toEqual([{ v: '@a' }]);
  });

  it('affected', async () => {
    const insert1 = await sql.query('INSERT INTO T (Name) VALUES (@n)', { n: 'a' });
    expect(insert1).toEqual({ rows: [], affected: 1 });

    await sql.query('INSERT INTO T (Name) VALUES (@n)', { n: 'b' });
    const update = await sql.query('UPDATE T SET Name = @n', { n: 'c' });
    expect(update.affected).toBe(2);

    const del = await sql.query('DELETE FROM T WHERE Id = 99');
    expect(del.affected).toBe(0);

    const select = await sql.query('SELECT * FROM T');
    expect(select.affected).toBe(select.rows.length);

    const returning = await sql.query<{ Id: number }>('INSERT INTO T (Name) VALUES (@n) RETURNING Id', { n: 'd' });
    expect(returning.rows).toHaveLength(1);
    expect(returning.affected).toBe(1);

    const withUpdate = await sql.query("WITH x AS (SELECT 1 AS a) UPDATE T SET Name = 'w' WHERE Id IN (SELECT a FROM x)");
    expect(withUpdate).toEqual({ rows: [], affected: 1 });
  });

  it('Normale Objekte', async () => {
    await sql.query('INSERT INTO T (Name) VALUES (@n)', { n: 'a' });
    const { rows } = await sql.query('SELECT * FROM T');
    expect(Object.getPrototypeOf(rows[0])).toBe(Object.prototype);
  });

  it('nur eine Anweisung', async () => {
    await expect(sql.query('SELECT 1; SELECT 2')).rejects.toEqual(expect.objectContaining({ code: 'sqlite-multi-statement' }));
    await expect(sql.query('SELECT 1;')).resolves.toBeDefined();
    await expect(sql.query("SELECT ';' AS s")).resolves.toBeDefined();
  });
});
