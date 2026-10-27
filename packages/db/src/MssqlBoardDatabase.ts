import sql from 'mssql';
import { HarnessError, type BoardConfig, type BoardUnitOfWork } from '@harness/core';
import { MssqlSchemaRunner } from './MssqlSchemaRunner.js';
import { MssqlUnitOfWork } from './MssqlUnitOfWork.js';
import { SqlErrorMapper } from './SqlErrorMapper.js';
import { SqlRunner } from './SqlRunner.js';
import type { ConnectOptions, ManagedBoardDatabase } from './ManagedBoardDatabase.js';

// Stand eines Boards: Ticketzahl (sinkt bei Löschungen) und höchste rowversion (datenbankweit steigend)
// von Tickets, Kommentaren, Notizen und dem Board selbst; dazu Zahl und höchste rowversion der Fragen,
// die Tickets des Boards betreffen (jede Änderung einer Frage erhöht ihre Rev und damit ihre rowversion);
// dazu die Zahl der Session-Verknüpfungen seiner Tickets
const CHANGE_MARKER_SQL = `
SELECT
  (SELECT COUNT_BIG(*) FROM board.Tickets WHERE BoardId = @boardId) AS Tickets,
  (SELECT MAX(CONVERT(bigint, RowVer)) FROM board.Tickets WHERE BoardId = @boardId) AS TicketVer,
  (SELECT MAX(CONVERT(bigint, c.RowVer)) FROM board.Comments c JOIN board.Tickets t ON t.Id = c.TicketId WHERE t.BoardId = @boardId) AS CommentVer,
  (SELECT MAX(CONVERT(bigint, RowVer)) FROM board.Notes WHERE BoardId = @boardId) AS NoteVer,
  (SELECT CONVERT(bigint, RowVer) FROM board.Boards WHERE Id = @boardId) AS BoardVer,
  (SELECT COUNT_BIG(*) FROM board.QuestionTickets qt JOIN board.Tickets t ON t.Id = qt.TicketId WHERE t.BoardId = @boardId) AS QuestionLinks,
  (SELECT MAX(CONVERT(bigint, q.RowVer)) FROM board.Questions q
     JOIN board.QuestionTickets qt ON qt.QuestionId = q.Id JOIN board.Tickets t ON t.Id = qt.TicketId
     WHERE t.BoardId = @boardId) AS QuestionVer,
  (SELECT COUNT_BIG(*) FROM board.TicketSessions ts JOIN board.Tickets t ON t.Id = ts.TicketId WHERE t.BoardId = @boardId) AS SessionLinks;`;

// Stand der Fragen eines Projekts: Anzahl und höchste rowversion
const QUESTION_MARKER_SQL = `
SELECT COUNT_BIG(*) AS Questions, MAX(CONVERT(bigint, RowVer)) AS QuestionVer FROM board.Questions WHERE ProjectId = @projectId;`;

type MarkerValue = string | number | null;

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

// Board-Datenbank auf SQL Server: ein Verbindungspool, jede Aktion in einer eigenen Transaktion.
export class MssqlBoardDatabase implements ManagedBoardDatabase {
  // Erweiterungen, die der DB fehlen und die auch nicht eingespielt werden konnten
  private missingFeatures: string[] = [];

  private constructor(
    private readonly pool: sql.ConnectionPool,
    private readonly database: string,
  ) {}

  // Verbindet mit der DB aus der Konfig; nicht erreichbar ergibt eine deutsche Meldung mit Server und DB
  static async connect(config: BoardConfig, options: ConnectOptions = {}): Promise<MssqlBoardDatabase> {
    const pool = new sql.ConnectionPool({
      server: config.server,
      port: config.port,
      database: config.database,
      user: config.user,
      password: config.password,
      options: { encrypt: config.encrypt, trustServerCertificate: config.trustServerCertificate },
      connectionTimeout: options.connectionTimeoutMs ?? 8000,
      requestTimeout: 30000,
      pool: { max: 5, min: 0 },
    });
    try {
      await pool.connect();
    } catch (error) {
      throw new HarnessError('board-db-unreachable', `Board-Datenbank ${config.database} auf ${config.server}:${config.port} ist nicht erreichbar: ${(error as Error).message}`);
    }
    return new MssqlBoardDatabase(pool, config.database);
  }

  // Führt die Arbeit in einer Transaktion aus; jeder Fehler rollt alles zurück, ein Deadlock wird zu 'conflict'
  async transaction<T>(work: (uow: BoardUnitOfWork) => Promise<T>): Promise<T> {
    const tx = new sql.Transaction(this.pool);
    await tx.begin(sql.ISOLATION_LEVEL.READ_COMMITTED);
    try {
      const result = await work(new MssqlUnitOfWork(new SqlRunner(tx), this.missingFeatures, this.database));
      await tx.commit();
      return result;
    } catch (error) {
      try {
        await tx.rollback();
      } catch {
        // Rollback kann scheitern, wenn SQL Server die Transaktion schon abgebrochen hat
      }
      throw SqlErrorMapper.map(error);
    }
  }

  // Höchste eingespielte Schema-Version; null, wenn das Schema board fehlt
  async schemaVersion(): Promise<number | null> {
    // Eigene Anweisungen je Zweig: so wird board.SchemaVersion nur aufgelöst, wenn die Tabelle existiert
    const result = await this.pool.request().query<{ V: number | null }>(
      "IF OBJECT_ID(N'board.SchemaVersion') IS NULL SELECT CAST(NULL AS int) AS V ELSE SELECT MAX(Version) AS V FROM board.SchemaVersion;",
    );
    return result.recordset[0]?.V ?? null;
  }

  // Eingespielte additive Erweiterungen (board.SchemaFeatures); leer, wenn es die Tabelle noch nicht gibt
  async schemaFeatures(): Promise<string[]> {
    const result = await this.pool.request().query<{ Name: string }>(
      "IF OBJECT_ID(N'board.SchemaFeatures') IS NULL SELECT CAST(NULL AS varchar(50)) AS Name WHERE 1 = 0 ELSE SELECT Name FROM board.SchemaFeatures;",
    );
    return result.recordset.map((r) => r.Name);
  }

  // Spielt fehlende Erweiterungen selbst ein (braucht db_ddladmin); ohne das Recht bleibt es bei einer Warnung
  ensureSchemaFeatures(missing: string[]): Promise<string[]> {
    return new MssqlSchemaRunner(this.pool).apply(missing);
  }

  // Erweiterungen, die weiter fehlen; die betroffenen Repositories werfen dann beim Zugriff
  setMissingFeatures(names: string[]): void {
    this.missingFeatures = names;
  }

  // Kennwert für den Stand eines Boards (außerhalb einer Transaktion, liest nur)
  async changeMarker(boardId: number): Promise<string> {
    const result = await this.pool.request().input('boardId', sql.Int, boardId).query<MarkerRow>(CHANGE_MARKER_SQL);
    const row = result.recordset[0];
    return this.join([row.Tickets, row.TicketVer, row.CommentVer, row.NoteVer, row.BoardVer, row.QuestionLinks, row.QuestionVer, row.SessionLinks]);
  }

  // Kennwert für den Stand der Fragen eines Projekts (außerhalb einer Transaktion, liest nur)
  async questionMarker(projectId: number): Promise<string> {
    const result = await this.pool.request().input('projectId', sql.Int, projectId).query<{ Questions: MarkerValue; QuestionVer: MarkerValue }>(QUESTION_MARKER_SQL);
    const row = result.recordset[0];
    return this.join([row.Questions, row.QuestionVer]);
  }

  // Werte als Text; null wird zu „-“
  private join(values: MarkerValue[]): string {
    return values.map((v) => (v === null ? '-' : String(v))).join(':');
  }

  // Schließt den Verbindungspool
  async close(): Promise<void> {
    await this.pool.close();
  }
}
