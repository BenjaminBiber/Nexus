import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { DatabaseSync, SQLInputValue } from 'node:sqlite';
import { NodeSqlite } from '../../../src/sqlite/NodeSqlite.js';
import { SqliteSchemaRunner } from '../../../src/sqlite/SqliteSchemaRunner.js';
import type { SqliteSchemaStep } from '../../../src/sqlite/schema/index.js';

/** Fremdschlüssel einer Spalte, wie PRAGMA foreign_key_list ihn meldet */
export interface ForeignKey {
  from: string;
  table: string;
  to: string;
  onDelete: string;
}

type Params = Record<string, SQLInputValue>;

// Frische SQLite-Datei mit den angegebenen Schema-Stufen und Lesehilfen für Struktur-Tests
export class SqliteSchemaInspect {
  static readonly NOW = '2026-10-21T08:00:00.000Z';
  static readonly USER = '11111111-1111-1111-1111-111111111111';

  private constructor(readonly db: DatabaseSync, readonly runner: SqliteSchemaRunner, readonly file: string, private readonly dir: string) {}

  // Legt eine Temp-Datei an und spielt die Stufen ein
  static fresh(steps: readonly SqliteSchemaStep[]): SqliteSchemaInspect {
    const dir = mkdtempSync(join(tmpdir(), 'harness-sqlite-'));
    const file = join(dir, 'board.db');
    const { DatabaseSync: Database } = NodeSqlite.load();
    const db = new Database(file);
    const runner = new SqliteSchemaRunner(db, steps);
    runner.run();
    return new SqliteSchemaInspect(db, runner, file, dir);
  }

  // Schließt die Datei (unter Windows Pflicht vor dem Löschen) und räumt den Ordner weg
  dispose(): void {
    this.db.close();
    rmSync(this.dir, { recursive: true, force: true });
  }

  // Tabellen ohne interne (sqlite_*) und ohne die Buchführung des Runners
  tables(): string[] {
    return this.all<{ name: string }>("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> 'SchemaSteps' ORDER BY name").map((r) => r.name);
  }

  // Spaltennamen in Tabellenreihenfolge
  columns(table: string): string[] {
    return this.all<{ name: string }>('SELECT name FROM pragma_table_info(@t) ORDER BY cid', { t: table }).map((r) => r.name);
  }

  // Eine Spalte mit Typ, NOT NULL (0/1) und Default-Ausdruck
  column(table: string, name: string): { type: string; notnull: number; dflt_value: string | null } | undefined {
    return this.all<{ type: string; notnull: number; dflt_value: string | null }>(
      'SELECT type, "notnull", dflt_value FROM pragma_table_info(@t) WHERE name = @n', { t: table, n: name })[0];
  }

  // Spalten des Primärschlüssels in Schlüsselreihenfolge
  primaryKey(table: string): string[] {
    return this.all<{ name: string }>('SELECT name FROM pragma_table_info(@t) WHERE pk > 0 ORDER BY pk', { t: table }).map((r) => r.name);
  }

  // Explizit angelegte Indizes (CREATE INDEX) mit ihren Spalten
  indexes(table: string): Record<string, string[]> {
    const result: Record<string, string[]> = {};
    for (const { name } of this.all<{ name: string }>("SELECT name FROM pragma_index_list(@t) WHERE origin = 'c'", { t: table })) {
      result[name] = this.indexColumns(name);
    }
    return result;
  }

  // Spaltenlisten der UNIQUE-Constraints, sortiert
  uniques(table: string): string[][] {
    return this.all<{ name: string }>("SELECT name FROM pragma_index_list(@t) WHERE origin = 'u'", { t: table })
      .map((r) => this.indexColumns(r.name))
      .sort((a, b) => a.join(',').localeCompare(b.join(',')));
  }

  // Fremdschlüssel der Tabelle, sortiert nach Spalte
  foreignKeys(table: string): ForeignKey[] {
    return this.all<{ from: string; table: string; to: string; on_delete: string }>('SELECT "from", "table", "to", on_delete FROM pragma_foreign_key_list(@t)', { t: table })
      .map((r) => ({ from: r.from, table: r.table, to: r.to, onDelete: r.on_delete }))
      .sort((a, b) => a.from.localeCompare(b.from));
  }

  // Namen aller Trigger
  triggers(): string[] {
    return this.all<{ name: string }>("SELECT name FROM sqlite_master WHERE type = 'trigger' ORDER BY name").map((r) => r.name);
  }

  // Verstöße gegen Fremdschlüssel in der ganzen Datei (leer = alles gültig)
  foreignKeyCheck(): unknown[] {
    return this.all('PRAGMA foreign_key_check');
  }

  // Führt eine Anweisung aus
  run(sql: string, params: Params = {}): void {
    this.db.prepare(sql).run(params);
  }

  // Zeilen einer Abfrage als normale Objekte
  all<T = Record<string, unknown>>(sql: string, params: Params = {}): T[] {
    return this.db.prepare(sql).all(params).map((r) => ({ ...r }) as T);
  }

  // Nutzer USER, Projekt AIH (Id 1) und Board (Id 1)
  seedBoard(): void {
    this.run("INSERT INTO Users (Id, Name) VALUES (@u, 'Test')", { u: SqliteSchemaInspect.USER });
    this.run(`INSERT INTO Projects ("Key", Name, CreatedAt, CreatedBy) VALUES ('AIH', 'Projekt', @now, @u)`, { now: SqliteSchemaInspect.NOW, u: SqliteSchemaInspect.USER });
    this.run("INSERT INTO Boards (ProjectId, OwnerId, Name, CreatedAt) VALUES (1, @u, 'Board', @now)", { now: SqliteSchemaInspect.NOW, u: SqliteSchemaInspect.USER });
  }

  // Ticket in Projekt 1 / Board 1 mit allen Pflichtfeldern
  insertTicket(id: string, number: number, status = 'backlog', priority = 'normal', boardId = 1): void {
    this.run(
      `INSERT INTO Tickets (Id, ProjectId, BoardId, Number, Title, Status, Priority, SortOrder, NeedsPlanning, CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev)
       VALUES (@id, 1, @boardId, @number, 'Titel', @status, @priority, @number, 0, @now, @u, 'Test', @now, 1)`,
      { id, number, status, priority, boardId, now: SqliteSchemaInspect.NOW, u: SqliteSchemaInspect.USER },
    );
  }

  private indexColumns(index: string): string[] {
    return this.all<{ name: string }>('SELECT name FROM pragma_index_info(@i) ORDER BY seqno', { i: index }).map((r) => r.name);
  }
}
