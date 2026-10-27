import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { DatabaseSync } from 'node:sqlite';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { NodeSqlite } from '../../src/sqlite/NodeSqlite.js';
import { SqliteSchemaRunner } from '../../src/sqlite/SqliteSchemaRunner.js';
import type { SqliteSchemaStep } from '../../src/sqlite/schema/index.js';

const { DatabaseSync: Database } = NodeSqlite.load();

const NOW = "(strftime('%Y-%m-%dT%H:%M:%fZ','now'))";
const v1: SqliteSchemaStep = {
  id: 't001',
  version: 1,
  sql: `CREATE TABLE SchemaVersion (Version INTEGER NOT NULL PRIMARY KEY, AppliedAt TEXT NOT NULL DEFAULT ${NOW}); CREATE TABLE A (X INTEGER);`,
};
const v2: SqliteSchemaStep = { id: 't002', version: 2, sql: 'CREATE TABLE B (X INTEGER);' };
// Zweite Anweisung scheitert, weil es A schon gibt
const broken: SqliteSchemaStep = { id: 't002', version: 2, sql: 'CREATE TABLE B (X INTEGER); CREATE TABLE A (Y INTEGER);' };
const feature: SqliteSchemaStep = {
  id: 't003',
  feature: 'f-test',
  sql: `CREATE TABLE SchemaFeatures (Name TEXT NOT NULL PRIMARY KEY, Script TEXT NOT NULL, AppliedAt TEXT NOT NULL DEFAULT ${NOW});`,
};

function count(db: DatabaseSync, table: string): number {
  return (db.prepare(`SELECT COUNT(*) AS N FROM ${table}`).get() as { N: number }).N;
}

function hasTable(db: DatabaseSync, name: string): boolean {
  return db.prepare("SELECT 1 AS X FROM sqlite_master WHERE type = 'table' AND name = ?").get(name) !== undefined;
}

describe('SqliteSchemaRunner', () => {
  let dir: string;
  let file: string;
  let db: DatabaseSync;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-sqlite-'));
    file = join(dir, 'board.db');
    db = new Database(file);
  });
  afterEach(() => {
    db.close();
    rmSync(dir, { recursive: true, force: true });
  });

  it('leere Datei hat keine Version und keine Erweiterungen', () => {
    const runner = new SqliteSchemaRunner(db, []);
    expect(runner.schemaVersion()).toBeNull();
    expect(runner.schemaFeatures()).toEqual([]);
  });

  it('frische Datei erreicht die letzte Version', () => {
    const runner = new SqliteSchemaRunner(db, [v1, v2]);
    expect(runner.run()).toEqual(['t001', 't002']);
    expect(runner.schemaVersion()).toBe(2);
    const ids = (db.prepare('SELECT Id FROM SchemaSteps ORDER BY Id').all() as { Id: string }[]).map((r) => r.Id);
    expect(ids).toEqual(['t001', 't002']);
    expect(hasTable(db, 'A')).toBe(true);
    expect(hasTable(db, 'B')).toBe(true);
  });

  it('zweites Öffnen führt nichts erneut aus', () => {
    new SqliteSchemaRunner(db, [v1, v2]).run();
    db.close();
    db = new Database(file);
    const runner = new SqliteSchemaRunner(db, [v1, v2]);
    expect(runner.run()).toEqual([]);
    expect(count(db, 'SchemaVersion')).toBe(2);
    expect(runner.schemaVersion()).toBe(2);
  });

  it('neue Stufe hinten wird nachgeholt', () => {
    expect(new SqliteSchemaRunner(db, [v1]).run()).toEqual(['t001']);
    const runner = new SqliteSchemaRunner(db, [v1, v2]);
    expect(runner.run()).toEqual(['t002']);
    expect(runner.schemaVersion()).toBe(2);
  });

  it('Fehler rollt nur diese Stufe zurück', () => {
    expect(() => new SqliteSchemaRunner(db, [v1, broken]).run()).toThrow(
      expect.objectContaining({ code: 'board-schema-failed', message: expect.stringContaining('t002') }),
    );
    expect(hasTable(db, 'B')).toBe(false);
    expect(hasTable(db, 'A')).toBe(true);
    const runner = new SqliteSchemaRunner(db, [v1, v2]);
    expect(runner.schemaVersion()).toBe(1);
    const ids = (db.prepare('SELECT Id FROM SchemaSteps ORDER BY Id').all() as { Id: string }[]).map((r) => r.Id);
    expect(ids).toEqual(['t001']);

    expect(runner.run()).toEqual(['t002']);
    expect(runner.schemaVersion()).toBe(2);
  });

  it('Feature wird eingetragen, ohne die Version zu erhöhen', () => {
    const runner = new SqliteSchemaRunner(db, [v1, feature]);
    runner.run();
    expect(runner.schemaFeatures()).toEqual(['f-test']);
    expect((db.prepare('SELECT Script FROM SchemaFeatures').get() as { Script: string }).Script).toBe('t003');
    expect(runner.schemaVersion()).toBe(1);
  });

  it('Standardliste läuft ohne Fehler', () => {
    expect(() => new SqliteSchemaRunner(db).run()).not.toThrow();
  });
});
