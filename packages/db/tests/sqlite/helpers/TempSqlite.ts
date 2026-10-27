import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { DatabaseSync } from 'node:sqlite';
import { NodeSqlite } from '../../../src/sqlite/NodeSqlite.js';
import { SqliteBoardDatabase } from '../../../src/sqlite/SqliteBoardDatabase.js';
import type { SqliteValue } from '../../../src/sqlite/SqliteRunner.js';

export const TEST_USER = { id: '11111111-1111-1111-1111-111111111111', name: 'Test' };
export const TEST_NOW = '2026-10-13T10:00:00.000Z';

export interface SeededBoard { userId: string; projectId: number; boardId: number }
export interface SeedTicket { id: string; projectId: number; boardId: number; number: number; title?: string; status?: string; sortOrder?: number }

// Temporäre SQLite-Board-Datei je Test: anlegen, befüllen, wegräumen.
export class TempSqlite {
  private readonly raws: DatabaseSync[] = [];

  private constructor(readonly dir: string, readonly file: string, readonly db: SqliteBoardDatabase) {}

  // Neuer Ordner unter os.tmpdir() (Präfix 'harness-sqlite-'), Datei board.db, Schema angelegt
  static async create(): Promise<TempSqlite> {
    const dir = mkdtempSync(join(tmpdir(), 'harness-sqlite-'));
    const file = join(dir, 'board.db');
    return new TempSqlite(dir, file, await SqliteBoardDatabase.open(file));
  }

  // Rohes SQL in eigener Transaktion (genau eine Anweisung)
  sql<T = Record<string, unknown>>(text: string, params: Record<string, SqliteValue> = {}): Promise<{ rows: T[]; affected: number }> {
    return this.db.runInTransaction((runner) => runner.query<T>(text, params));
  }

  // Legt TEST_USER (oder einen anderen) an, falls es ihn noch nicht gibt; liefert die Id
  async seedUser(user: { id: string; name: string } = TEST_USER): Promise<string> {
    await this.sql('INSERT INTO Users (Id, Name) VALUES (@id, @name) ON CONFLICT(Id) DO NOTHING', { id: user.id, name: user.name });
    return user.id;
  }

  // Legt TEST_USER und ein Projekt an; liefert die Projekt-Id
  async seedProject(key = 'ARC'): Promise<number> {
    const userId = await this.seedUser();
    const { rows } = await this.sql<{ Id: number }>(
      'INSERT INTO Projects ("Key", Name, CreatedAt, CreatedBy) VALUES (@key, @name, @createdAt, @createdBy) RETURNING Id',
      { key, name: `Projekt ${key}`, createdAt: TEST_NOW, createdBy: userId },
    );
    return Number(rows[0].Id);
  }

  // Legt ein Board an (ohne projectId zuerst Benutzer und Projekt 'ARC')
  async seedBoard(projectId?: number, name = 'Sprint'): Promise<SeededBoard> {
    const pid = projectId ?? await this.seedProject();
    const userId = await this.seedUser();
    const { rows } = await this.sql<{ Id: number }>(
      'INSERT INTO Boards (ProjectId, OwnerId, Name, RepoPath, CreatedAt) VALUES (@projectId, @ownerId, @name, NULL, @createdAt) RETURNING Id',
      { projectId: pid, ownerId: userId, name, createdAt: TEST_NOW },
    );
    return { userId, projectId: pid, boardId: Number(rows[0].Id) };
  }

  // Legt ein Ticket mit Mindestfeldern an (Rest über Spalten-Defaults); Ersteller ist TEST_USER
  async seedTicket(t: SeedTicket): Promise<void> {
    await this.sql(
      `INSERT INTO Tickets (Id, ProjectId, BoardId, Number, Title, Status, Priority, SortOrder, NeedsPlanning,
         CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev)
       VALUES (@id, @projectId, @boardId, @number, @title, @status, 'normal', @sortOrder, 0,
         @now, @userId, @userName, @now, 1)`,
      { id: t.id, projectId: t.projectId, boardId: t.boardId, number: t.number, title: t.title ?? 'Titel',
        status: t.status ?? 'backlog', sortOrder: t.sortOrder ?? 0, now: TEST_NOW, userId: TEST_USER.id, userName: TEST_USER.name },
    );
  }

  // Zweite Verbindung auf dieselbe Datei für rohes SQL (wird in dispose() geschlossen)
  openRaw(): DatabaseSync {
    const { DatabaseSync } = NodeSqlite.load();
    const raw = new DatabaseSync(this.file);
    raw.exec('PRAGMA busy_timeout = 10000;');
    raw.exec('PRAGMA foreign_keys = ON;');
    this.raws.push(raw);
    return raw;
  }

  // Schließt Datenbank und Roh-Verbindungen und löscht den Ordner
  async dispose(): Promise<void> {
    for (const raw of this.raws.splice(0)) {
      try { raw.close(); } catch { /* vom Test schon geschlossen */ }
    }
    await this.db.close();
    rmSync(this.dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 });
  }
}
