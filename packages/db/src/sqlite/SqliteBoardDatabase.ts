import { mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import type { DatabaseSync } from 'node:sqlite';
import { HarnessError, type BoardDatabase, type BoardUnitOfWork } from '@harness/core';
import { SqlErrorMapper } from '../SqlErrorMapper.js';
import { NodeSqlite } from './NodeSqlite.js';
import { SqliteRunner } from './SqliteRunner.js';
import { SqliteSchemaRunner } from './SqliteSchemaRunner.js';
import { SqliteSearchRepository } from './SqliteSearchRepository.js';
import { SqliteUnitOfWork } from './SqliteUnitOfWork.js';

// Stand eines Boards: Ticketzahl und höchster RowVer-Zähler (Trigger, datenbankweit steigend) von Tickets,
// Kommentaren, Notizen und dem Board selbst; dazu Zahl und höchster Zähler der Fragen zu seinen Tickets
// sowie die Zahl der Session-Verknüpfungen seiner Tickets. Übersetzung von MssqlBoardDatabase (ohne board.).
const CHANGE_MARKER_SQL = `
SELECT
  (SELECT COUNT(*) FROM Tickets WHERE BoardId = @boardId) AS Tickets,
  (SELECT MAX(RowVer) FROM Tickets WHERE BoardId = @boardId) AS TicketVer,
  (SELECT MAX(c.RowVer) FROM Comments c JOIN Tickets t ON t.Id = c.TicketId WHERE t.BoardId = @boardId) AS CommentVer,
  (SELECT MAX(RowVer) FROM Notes WHERE BoardId = @boardId) AS NoteVer,
  (SELECT RowVer FROM Boards WHERE Id = @boardId) AS BoardVer,
  (SELECT COUNT(*) FROM QuestionTickets qt JOIN Tickets t ON t.Id = qt.TicketId WHERE t.BoardId = @boardId) AS QuestionLinks,
  (SELECT MAX(q.RowVer) FROM Questions q
     JOIN QuestionTickets qt ON qt.QuestionId = q.Id JOIN Tickets t ON t.Id = qt.TicketId
     WHERE t.BoardId = @boardId) AS QuestionVer,
  (SELECT COUNT(*) FROM TicketSessions ts JOIN Tickets t ON t.Id = ts.TicketId WHERE t.BoardId = @boardId) AS SessionLinks`;

// Stand der Fragen eines Projekts: Anzahl und höchster Zähler
const QUESTION_MARKER_SQL = 'SELECT COUNT(*) AS Questions, MAX(RowVer) AS QuestionVer FROM Questions WHERE ProjectId = @projectId';

type MarkerValue = string | number | bigint | null;

interface MarkerRow {
  Tickets: MarkerValue;
  TicketVer: MarkerValue;
  CommentVer: MarkerValue;
  NoteVer: MarkerValue;
  BoardVer: MarkerValue;
  QuestionLinks: MarkerValue;
  QuestionVer: MarkerValue;
  SessionLinks: MarkerValue;
}

// Board-Datenbank in einer SQLite-Datei: eine Verbindung, alle Zugriffe einer Datei laufen nacheinander.
export class SqliteBoardDatabase implements BoardDatabase {
  // Warteschlange je Datei und Prozess: node:sqlite wartet synchron auf eine gesperrte Datei; zwei Instanzen
  // auf derselben Datei würden sonst die Ereignisschleife blockieren und SQLITE_BUSY bekommen.
  private static readonly queues = new Map<string, Promise<unknown>>();

  private readonly key: string;
  private closed = false;

  private constructor(
    private readonly db: DatabaseSync,
    readonly file: string,
  ) {
    this.key = SqliteBoardDatabase.queueKey(file);
  }

  // Öffnet (oder erzeugt) die Datei samt fehlendem Ordner, setzt die PRAGMAs und legt fehlende Schema-Stufen an
  static async open(file: string): Promise<SqliteBoardDatabase> {
    let db: DatabaseSync;
    try {
      if (file !== ':memory:') mkdirSync(dirname(file), { recursive: true });
      const { DatabaseSync } = NodeSqlite.load();
      db = new DatabaseSync(file);
    } catch (error) {
      throw new HarnessError('board-db-unreachable', `Board-Datei ${file} lässt sich nicht öffnen: ${(error as Error).message}`);
    }
    try {
      // busy_timeout zuerst: das Umschalten auf WAL und die Schema-Stufen brauchen eine exklusive Sperre und
      // würden sonst sofort mit SQLITE_BUSY scheitern, wenn ein anderer Prozess die Datei gerade schreibt.
      db.exec('PRAGMA busy_timeout = 10000;');
      db.exec('PRAGMA journal_mode = WAL;');
      db.exec('PRAGMA foreign_keys = ON;');
      SqliteSearchRepository.register(db);
      new SqliteSchemaRunner(db).run();
    } catch (error) {
      db.close();
      throw SqlErrorMapper.map(error);
    }
    return new SqliteBoardDatabase(db, file);
  }

  // Schlüssel der gemeinsamen Warteschlange; unter Windows zählt Groß-/Kleinschreibung des Pfades nicht
  private static queueKey(file: string): string {
    const full = file === ':memory:' ? file : resolve(file);
    return process.platform === 'win32' ? full.toLowerCase() : full;
  }

  // Hängt eine Aktion an die Warteschlange der Datei; ein Fehler blockiert die folgenden nicht
  private enqueue<T>(action: () => Promise<T>): Promise<T> {
    const previous = SqliteBoardDatabase.queues.get(this.key) ?? Promise.resolve();
    const run = previous.then(() => {
      if (this.closed) throw new HarnessError('board-db-closed', 'Die Board-Datenbank ist bereits geschlossen.');
      return action();
    });
    SqliteBoardDatabase.queues.set(this.key, run.catch(() => undefined));
    return run;
  }

  // Führt Arbeit mit dem rohen SqliteRunner in einer Transaktion aus (Grundlage von transaction(), auch für Tests).
  // Erst aufgeschoben (BEGIN): reine Lesezugriffe nehmen so keine Schreibsperre und warten nie auf einen anderen Prozess.
  // Will die Arbeit danach schreiben, während ein anderer Prozess die Datei hält oder schon weitergeschrieben hat, meldet
  // SQLite sofort SQLITE_BUSY; dann läuft sie genau einmal neu, diesmal mit sofortiger Schreibsperre (BEGIN IMMEDIATE).
  runInTransaction<T>(work: (sql: SqliteRunner) => Promise<T>): Promise<T> {
    return this.enqueue(async () => {
      try {
        return await this.attempt('BEGIN', work);
      } catch (error) {
        if (!SqlErrorMapper.isSqliteBusy(error)) throw SqlErrorMapper.map(error);
      }
      try {
        return await this.attempt('BEGIN IMMEDIATE', work);
      } catch (error) {
        throw SqlErrorMapper.map(error);
      }
    });
  }

  // Ein Durchlauf der Arbeit in einer Transaktion; jeder Fehler rollt zurück und wird unverändert weitergeworfen
  private async attempt<T>(begin: string, work: (sql: SqliteRunner) => Promise<T>): Promise<T> {
    this.db.exec(begin);
    try {
      const result = await work(new SqliteRunner(this.db));
      this.db.exec('COMMIT');
      return result;
    } catch (error) {
      try {
        if (this.db.isTransaction) this.db.exec('ROLLBACK');
      } catch {
        // Rollback kann scheitern, wenn SQLite die Transaktion schon beendet hat
      }
      throw error;
    }
  }

  // Jede fachliche Aktion in genau einer Transaktion; jeder Fehler rollt alles zurück
  transaction<T>(work: (uow: BoardUnitOfWork) => Promise<T>): Promise<T> {
    return this.runInTransaction((sql) => work(new SqliteUnitOfWork(sql)));
  }

  // Höchste eingespielte Schema-Version; null, solange es SchemaVersion nicht gibt
  schemaVersion(): Promise<number | null> {
    return this.enqueue(async () => new SqliteSchemaRunner(this.db).schemaVersion());
  }

  // Eingespielte additive Erweiterungen; leer, solange es SchemaFeatures nicht gibt
  schemaFeatures(): Promise<string[]> {
    return this.enqueue(async () => new SqliteSchemaRunner(this.db).schemaFeatures());
  }

  // Nichts zu tun: der SqliteSchemaRunner hat beim Öffnen schon alle Stufen und Erweiterungen angelegt
  async ensureSchemaFeatures(): Promise<string[]> {
    return [];
  }

  // Nichts zu merken: bei SQLite kann nach dem Öffnen keine Erweiterung fehlen
  setMissingFeatures(): void {}

  // Kennwert für den Stand eines Boards (liest nur, außerhalb einer Transaktion)
  changeMarker(boardId: number): Promise<string> {
    return this.enqueue(async () => {
      const { rows } = await new SqliteRunner(this.db).query<MarkerRow>(CHANGE_MARKER_SQL, { boardId });
      const row = rows[0];
      return this.join([row.Tickets, row.TicketVer, row.CommentVer, row.NoteVer, row.BoardVer, row.QuestionLinks, row.QuestionVer, row.SessionLinks]);
    });
  }

  // Kennwert für den Stand der Fragen eines Projekts (liest nur, außerhalb einer Transaktion)
  questionMarker(projectId: number): Promise<string> {
    return this.enqueue(async () => {
      const { rows } = await new SqliteRunner(this.db).query<{ Questions: MarkerValue; QuestionVer: MarkerValue }>(QUESTION_MARKER_SQL, { projectId });
      const row = rows[0];
      return this.join([row.Questions, row.QuestionVer]);
    });
  }

  // Werte als Text; null wird zu „-“
  private join(values: MarkerValue[]): string {
    return values.map((v) => (v === null ? '-' : String(v))).join(':');
  }

  // Wartet auf laufende Aktionen und schließt die Verbindung; mehrfacher Aufruf schadet nicht
  async close(): Promise<void> {
    if (this.closed) return;
    await SqliteBoardDatabase.queues.get(this.key);
    if (this.closed) return;
    this.closed = true;
    this.db.close();
  }
}
