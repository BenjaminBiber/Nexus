import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { SchemaVersionGuard } from '../../src/SchemaVersionGuard.js';
import { SqliteBoardDatabase } from '../../src/sqlite/SqliteBoardDatabase.js';
import { TempSqlite, TEST_NOW, TEST_USER } from './helpers/TempSqlite.js';

describe('SqliteBoardDatabase', () => {
  let temp: TempSqlite;

  beforeEach(async () => {
    temp = await TempSqlite.create();
  });
  afterEach(async () => {
    await temp.dispose();
  });

  it('legt Ordner und Datei an und setzt die PRAGMAs', async () => {
    const nested = join(temp.dir, 'a', 'b', 'board.db');
    const db = await SqliteBoardDatabase.open(nested);
    expect(existsSync(nested)).toBe(true);
    await db.close();

    const journal = await temp.sql<{ journal_mode: string }>('PRAGMA journal_mode');
    expect(journal.rows[0].journal_mode).toBe('wal');
    const keys = await temp.sql<{ foreign_keys: number }>('PRAGMA foreign_keys');
    expect(Number(keys.rows[0].foreign_keys)).toBe(1);
    const busy = await temp.sql<{ timeout: number }>('PRAGMA busy_timeout');
    expect(Number(busy.rows[0].timeout)).toBe(10000);
  });

  it('meldet Schema-Version 3 und das Feature dispatch-runs', async () => {
    const version = await temp.db.schemaVersion();
    const features = await temp.db.schemaFeatures();
    expect(version).toBe(3);
    expect(features).toContain('dispatch-runs');
    expect(() => new SchemaVersionGuard().check(version, temp.file, features)).not.toThrow();
  });

  it('öffnet eine vorhandene Datei erneut ohne Datenverlust', async () => {
    await temp.seedUser();
    await temp.db.close();

    const again = await SqliteBoardDatabase.open(temp.file);
    const { rows } = await again.runInTransaction((sql) => sql.query<{ N: number }>('SELECT COUNT(*) AS N FROM Users'));
    expect(Number(rows[0].N)).toBe(1);
    expect(await again.schemaVersion()).toBe(3);
    await again.close();
  });

  it('führt gleichzeitige Transaktionen nacheinander aus', async () => {
    const order: string[] = [];
    const a = temp.db.transaction(async () => {
      order.push('a1');
      await new Promise((r) => setTimeout(r, 30));
      order.push('a2');
    });
    const b = temp.db.transaction(async () => {
      order.push('b1');
      order.push('b2');
    });
    await Promise.all([a, b]);
    expect(order).toEqual(['a1', 'a2', 'b1', 'b2']);
  });

  it('serialisiert zwei Instanzen auf derselben Datei, ohne die Ereignisschleife zu blockieren', async () => {
    const second = await SqliteBoardDatabase.open(temp.file);
    let ticks = 0;
    const timer = setInterval(() => { ticks += 1; }, 10);
    try {
      const a = temp.db.runInTransaction(async (sql) => {
        await sql.query('INSERT INTO Users (Id, Name) VALUES (@id, @name)', { id: TEST_USER.id, name: 'A' });
        await new Promise((r) => setTimeout(r, 200));
      });
      const b = second.runInTransaction((sql) =>
        sql.query('INSERT INTO Users (Id, Name) VALUES (@id, @name)', { id: '22222222-2222-2222-2222-222222222222', name: 'B' }));
      await Promise.all([a, b]);
      expect(ticks).toBeGreaterThan(5);
      const { rows } = await temp.sql<{ N: number }>('SELECT COUNT(*) AS N FROM Users');
      expect(Number(rows[0].N)).toBe(2);
    } finally {
      clearInterval(timer);
      await second.close();
    }
  });

  it('liest, ohne auf die Schreibsperre eines anderen Prozesses zu warten', async () => {
    const { DatabaseSync } = await import('node:sqlite');
    const other = new DatabaseSync(temp.file);
    try {
      other.exec('BEGIN IMMEDIATE');
      const started = Date.now();
      const { rows } = await temp.db.runInTransaction((sql) => sql.query<{ N: number }>('SELECT COUNT(*) AS N FROM Users'));
      expect(Number(rows[0].N)).toBe(0);
      expect(Date.now() - started).toBeLessThan(2000);
      other.exec('ROLLBACK');
    } finally {
      other.close();
    }
  });

  it('wiederholt eine Arbeit mit Schreibsperre, wenn ein anderer Prozess nach ihrem Lesen geschrieben hat', async () => {
    const { DatabaseSync } = await import('node:sqlite');
    const other = new DatabaseSync(temp.file);
    let runs = 0;
    try {
      await temp.db.runInTransaction(async (sql) => {
        runs += 1;
        await sql.query('SELECT COUNT(*) AS N FROM Users');
        if (runs === 1) other.exec("INSERT INTO Users (Id, Name) VALUES ('22222222-2222-2222-2222-222222222222', 'B')");
        await sql.query('INSERT INTO Users (Id, Name) VALUES (@id, @name)', { id: TEST_USER.id, name: 'A' });
      });
    } finally {
      other.close();
    }
    expect(runs).toBe(2);
    const { rows } = await temp.sql<{ N: number }>('SELECT COUNT(*) AS N FROM Users');
    expect(Number(rows[0].N)).toBe(2);
  });

  it('rollt bei einem Fehler zurück', async () => {
    await expect(temp.db.runInTransaction(async (sql) => {
      await sql.query('INSERT INTO Users (Id, Name) VALUES (@id, @name)', { id: TEST_USER.id, name: 'X' });
      throw new Error('boom');
    })).rejects.toThrow('boom');

    const { rows } = await temp.sql<{ N: number }>('SELECT COUNT(*) AS N FROM Users');
    expect(Number(rows[0].N)).toBe(0);
    await expect(temp.seedUser()).resolves.toBe(TEST_USER.id);
  });

  it('gibt nach einem SQL-Fehler die Verbindung frei', async () => {
    const board = await temp.seedBoard();
    await expect(temp.seedTicket({ id: 'ARC-1', projectId: board.projectId, boardId: board.boardId, number: 1, status: 'falsch' })).rejects.toThrow();
    await expect(temp.seedTicket({ id: 'ARC-1', projectId: board.projectId, boardId: board.boardId, number: 1 })).resolves.toBeUndefined();
  });

  it('liefert eine UnitOfWork mit allen 17 Repositories', async () => {
    const keys = await temp.db.transaction(async (uow) => Object.keys(uow).sort());
    expect(keys).toEqual([
      'boards', 'comments', 'deletions', 'dispatchRuns', 'dispatchStats', 'history', 'notes', 'projects',
      'questionComments', 'questionHistory', 'questionResearch', 'questions', 'search', 'steps',
      'ticketSessions', 'tickets', 'users',
    ]);
  });

  it('ändert changeMarker nach Insert, Update und Delete, nicht bei anderen Boards', async () => {
    const a = await temp.seedBoard();
    const b = await temp.seedBoard(a.projectId, 'Andere');
    const m0 = await temp.db.changeMarker(a.boardId);
    const mb = await temp.db.changeMarker(b.boardId);
    expect(m0).toMatch(/^0:-:-:-:\d+:0:-:0$/);

    await temp.seedTicket({ id: 'ARC-1', projectId: a.projectId, boardId: a.boardId, number: 1 });
    const m1 = await temp.db.changeMarker(a.boardId);
    expect(m1).not.toBe(m0);

    await temp.sql("UPDATE Tickets SET Title = 'Neu' WHERE Id = 'ARC-1'");
    const m2 = await temp.db.changeMarker(a.boardId);
    expect(m2).not.toBe(m1);

    await temp.sql(
      "INSERT INTO Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt) VALUES ('ARC-1', @u, 'Test', 'Hallo', @now)",
      { u: a.userId, now: TEST_NOW },
    );
    const m3 = await temp.db.changeMarker(a.boardId);
    expect(m3).not.toBe(m2);

    await temp.sql("DELETE FROM Tickets WHERE Id = 'ARC-1'");
    const m4 = await temp.db.changeMarker(a.boardId);
    expect(m4).not.toBe(m3);

    expect(await temp.db.changeMarker(b.boardId)).toBe(mb);
  });

  it('lässt den Marker bei Rollback unverändert', async () => {
    const a = await temp.seedBoard();
    const m0 = await temp.db.changeMarker(a.boardId);
    await expect(temp.db.runInTransaction(async (sql) => {
      await sql.query(
        `INSERT INTO Tickets (Id, ProjectId, BoardId, Number, Title, Status, Priority, SortOrder, NeedsPlanning,
           CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev)
         VALUES ('ARC-9', @projectId, @boardId, 9, 'Titel', 'backlog', 'normal', 0, 0, @now, @userId, @userName, @now, 1)`,
        { projectId: a.projectId, boardId: a.boardId, now: TEST_NOW, userId: TEST_USER.id, userName: TEST_USER.name },
      );
      throw new Error('boom');
    })).rejects.toThrow('boom');
    expect(await temp.db.changeMarker(a.boardId)).toBe(m0);
  });

  it('ändert questionMarker mit den Fragen eines Projekts', async () => {
    const projectId = await temp.seedProject();
    expect(await temp.db.questionMarker(projectId)).toBe('0:-');

    await temp.sql(
      `INSERT INTO Questions (Id, ProjectId, Number, Title, State, OriginKind, OriginName, CreatedAt, CreatedById, UpdatedAt, Rev)
       VALUES ('ARC-Q1', @p, 1, 'Frage', 'open', 'human', 'Test', @now, @u, @now, 1)`,
      { p: projectId, now: TEST_NOW, u: TEST_USER.id },
    );
    const m1 = await temp.db.questionMarker(projectId);
    expect(m1).toMatch(/^1:\d+$/);

    await temp.sql("UPDATE Questions SET Title = 'Neu' WHERE Id = 'ARC-Q1'");
    expect(await temp.db.questionMarker(projectId)).not.toBe(m1);
  });

  it('lässt sich mehrfach schließen und meldet danach board-db-closed', async () => {
    const db = await SqliteBoardDatabase.open(join(temp.dir, 'x.db'));
    await db.close();
    await db.close();
    await expect(db.transaction(async () => 1)).rejects.toMatchObject({ code: 'board-db-closed' });
  });

  it('lässt eine Roh-Verbindung bestätigte Daten sehen', async () => {
    await temp.seedUser();
    const raw = temp.openRaw();
    expect(Number((raw.prepare('SELECT COUNT(*) AS N FROM Users').get() as { N: number }).N)).toBe(1);
    expect(Number((raw.prepare('PRAGMA foreign_keys').get() as { foreign_keys: number }).foreign_keys)).toBe(1);
  });

  it('meldet eine nicht zu öffnende Datei verständlich', async () => {
    await expect(SqliteBoardDatabase.open(temp.dir)).rejects.toMatchObject({ code: 'board-db-unreachable' });
  });
});
