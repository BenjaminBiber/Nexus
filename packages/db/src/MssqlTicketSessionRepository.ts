import type { TicketSessionRepository } from '@harness/core';
import { SqlChunks } from './SqlChunks.js';
import { SqlRunner } from './SqlRunner.js';

// Verknüpfung von Claude-Sessions mit Tickets in board.TicketSessions.
export class MssqlTicketSessionRepository implements TicketSessionRepository {
  constructor(private readonly sql: SqlRunner) {}

  // Legt die Verknüpfung nur an, wenn es sie noch nicht gibt (Sperre gegen gleichzeitiges Anlegen)
  async add(ticketId: string, sessionId: string, at: string): Promise<boolean> {
    const { affected } = await this.sql.query(
      `INSERT INTO board.TicketSessions (TicketId, SessionId, AddedAt)
       SELECT @ticketId, @sessionId, @at
       WHERE NOT EXISTS (SELECT 1 FROM board.TicketSessions WITH (UPDLOCK, HOLDLOCK) WHERE TicketId = @ticketId AND SessionId = @sessionId)`,
      { ticketId, sessionId, at: SqlRunner.date(at) },
    );
    return affected > 0;
  }

  async byTickets(ticketIds: string[]): Promise<Record<string, string[]>> {
    const result: Record<string, string[]> = {};
    for (const chunk of SqlChunks.of(ticketIds)) {
      const { list, params } = SqlChunks.params('t', chunk);
      const { rows } = await this.sql.query<{ TicketId: string; SessionId: string }>(
        `SELECT TicketId, SessionId FROM board.TicketSessions WHERE TicketId IN (${list}) ORDER BY AddedAt, SessionId`,
        params,
      );
      for (const r of rows) (result[r.TicketId] ??= []).push(r.SessionId);
    }
    return result;
  }

  // Sortiert in JS, damit die Reihenfolge nicht von der DB-Collation abhängt
  async ticketsOf(sessionId: string): Promise<string[]> {
    const { rows } = await this.sql.query<{ TicketId: string }>('SELECT TicketId FROM board.TicketSessions WHERE SessionId = @sessionId', { sessionId });
    return rows.map((r) => r.TicketId).sort();
  }
}
