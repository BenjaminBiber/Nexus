import type { TicketSessionRepository } from '@harness/core';
import { SqlChunks } from '../SqlChunks.js';
import { SqliteRunner } from './SqliteRunner.js';

// Verknüpfung von Claude-Sessions mit Tickets in TicketSessions.
export class SqliteTicketSessionRepository implements TicketSessionRepository {
  constructor(private readonly sql: SqliteRunner) {}

  // Legt die Verknüpfung nur an, wenn es sie noch nicht gibt (Primärschlüssel TicketId+SessionId)
  async add(ticketId: string, sessionId: string, at: string): Promise<boolean> {
    const { affected } = await this.sql.query(
      `INSERT INTO TicketSessions (TicketId, SessionId, AddedAt)
       VALUES (@ticketId, @sessionId, @at)
       ON CONFLICT(TicketId, SessionId) DO NOTHING`,
      { ticketId, sessionId, at: new Date(at) },
    );
    return affected > 0;
  }

  async byTickets(ticketIds: string[]): Promise<Record<string, string[]>> {
    const result: Record<string, string[]> = {};
    for (const chunk of SqlChunks.of(ticketIds)) {
      const { list, params } = SqlChunks.params('t', chunk);
      const { rows } = await this.sql.query<{ TicketId: string; SessionId: string }>(
        `SELECT TicketId, SessionId FROM TicketSessions WHERE TicketId IN (${list}) ORDER BY AddedAt, SessionId`,
        params,
      );
      for (const r of rows) (result[r.TicketId] ??= []).push(r.SessionId);
    }
    return result;
  }

  // Sortiert in JS, damit die Reihenfolge nicht von der DB-Collation abhängt
  async ticketsOf(sessionId: string): Promise<string[]> {
    const { rows } = await this.sql.query<{ TicketId: string }>('SELECT TicketId FROM TicketSessions WHERE SessionId = @sessionId', { sessionId });
    return rows.map((r) => r.TicketId).sort();
  }
}
