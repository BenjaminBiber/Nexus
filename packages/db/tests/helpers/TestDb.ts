import { describe } from 'vitest';
import sql from 'mssql';
import type { BoardConfig } from '@harness/core';

const USER_ID = '11111111-1111-1111-1111-111111111111';

// Zugang zur Test-Datenbank aus HARNESS_TEST_DB; ohne Variable werden DB-Tests übersprungen.
export class TestDb {
  // Konfig aus der Umgebungsvariable oder null
  static config(): BoardConfig | null {
    const raw = process.env.HARNESS_TEST_DB;
    if (!raw) return null;
    const data = JSON.parse(raw) as Partial<BoardConfig>;
    if (!data.database?.endsWith('_Test')) throw new Error('HARNESS_TEST_DB muss auf eine Datenbank mit Endung _Test zeigen.');
    return {
      // file wird nur bei provider sqlite genutzt
      provider: 'mssql', file: '',
      server: data.server!, port: data.port ?? 1433, database: data.database, user: data.user!, password: data.password!,
      encrypt: data.encrypt ?? true, trustServerCertificate: data.trustServerCertificate ?? true,
      userId: USER_ID, userName: 'Test', activeBoard: null,
    };
  }

  // describe oder describe.skip, je nachdem ob eine Test-DB eingerichtet ist
  static get describe(): typeof describe | typeof describe.skip {
    return TestDb.config() ? describe : describe.skip;
  }

  // Leert alle Tabellen des Schemas board (Reihenfolge wegen Fremdschlüsseln)
  static async reset(): Promise<void> {
    const config = TestDb.config()!;
    const pool = await new sql.ConnectionPool({
      server: config.server, port: config.port, database: config.database, user: config.user, password: config.password,
      options: { encrypt: config.encrypt, trustServerCertificate: config.trustServerCertificate },
    }).connect();
    try {
      await pool.request().batch(`
        DELETE FROM board.QuestionOptions; DELETE FROM board.QuestionResearch; DELETE FROM board.QuestionComments;
        DELETE FROM board.QuestionHistory; DELETE FROM board.QuestionDependencies; DELETE FROM board.QuestionTickets;
        DELETE FROM board.QuestionTags; DELETE FROM board.Questions;
        DELETE FROM board.TicketSteps; DELETE FROM board.TicketSessions; DELETE FROM board.DispatchRuns;
        DELETE FROM board.History; DELETE FROM board.Comments; DELETE FROM board.TicketLabels;
        DELETE FROM board.TicketDependencies; DELETE FROM board.Notes; DELETE FROM board.Tickets;
        DELETE FROM board.Boards; DELETE FROM board.Projects; DELETE FROM board.Deletions; DELETE FROM board.Users;`);
    } finally {
      await pool.close();
    }
  }
}
