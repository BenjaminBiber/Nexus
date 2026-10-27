import type { DatabaseSync } from 'node:sqlite';
import { HarnessError } from '@harness/core';
import { SQLITE_SCHEMA_STEPS, type SqliteSchemaStep } from './schema/index.js';

// Buchführung des Runners: welche Stufen schon eingespielt sind
const STEPS_TABLE =
  "CREATE TABLE IF NOT EXISTS SchemaSteps (Id TEXT NOT NULL PRIMARY KEY, AppliedAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')))";

// Legt das Schema einer SQLite-Datei an bzw. ergänzt fehlende Stufen; pflegt SchemaVersion und SchemaFeatures
// so, dass SchemaVersionGuard unverändert prüfen kann.
export class SqliteSchemaRunner {
  constructor(
    private readonly db: DatabaseSync,
    private readonly steps: readonly SqliteSchemaStep[] = SQLITE_SCHEMA_STEPS,
  ) {}

  // Führt alle fehlenden Stufen der Reihe nach aus, jede in einer eigenen Transaktion; liefert die Ids der ausgeführten
  run(): string[] {
    this.db.exec(STEPS_TABLE);
    const applied: string[] = [];
    for (const step of this.steps) {
      if (!this.isApplied(step.id) && this.apply(step)) applied.push(step.id);
    }
    return applied;
  }

  // Höchste eingetragene Schema-Version; null, solange es SchemaVersion nicht gibt
  schemaVersion(): number | null {
    if (!this.hasTable('SchemaVersion')) return null;
    const row = this.db.prepare('SELECT MAX(Version) AS V FROM SchemaVersion').get() as
      | { V: number | null }
      | undefined;
    return row?.V ?? null;
  }

  // Eingetragene additive Erweiterungen; leer, solange es SchemaFeatures nicht gibt
  schemaFeatures(): string[] {
    if (!this.hasTable('SchemaFeatures')) return [];
    const rows = this.db.prepare('SELECT Name FROM SchemaFeatures ORDER BY Name').all() as { Name: string }[];
    return rows.map((r) => r.Name);
  }

  // Eine Stufe in einer Transaktion; false, wenn ein anderer Prozess sie inzwischen eingespielt hat
  private apply(step: SqliteSchemaStep): boolean {
    this.db.exec('BEGIN IMMEDIATE');
    try {
      if (this.isApplied(step.id)) {
        this.db.exec('COMMIT');
        return false;
      }
      this.db.exec(step.sql);
      if (step.version !== undefined)
        this.db.prepare('INSERT INTO SchemaVersion (Version) VALUES (?)').run(step.version);
      if (step.feature !== undefined)
        this.db.prepare('INSERT INTO SchemaFeatures (Name, Script) VALUES (?, ?)').run(step.feature, step.id);
      this.db.prepare('INSERT INTO SchemaSteps (Id) VALUES (?)').run(step.id);
      this.db.exec('COMMIT');
      return true;
    } catch (error) {
      try {
        this.db.exec('ROLLBACK');
      } catch {
        // SQLite kann die Transaktion bei manchen Fehlern schon selbst beendet haben
      }
      throw new HarnessError(
        'board-schema-failed',
        `Schema-Stufe ${step.id} der SQLite-Datei ist gescheitert: ${(error as Error).message}`,
      );
    }
  }

  private isApplied(id: string): boolean {
    return this.db.prepare('SELECT 1 AS X FROM SchemaSteps WHERE Id = ?').get(id) !== undefined;
  }

  private hasTable(name: string): boolean {
    return this.db.prepare("SELECT 1 AS X FROM sqlite_master WHERE type = 'table' AND name = ?").get(name) !== undefined;
  }
}
