import type { BoardHistoryEntry, BoardHistoryRepository } from '@harness/core';
import { SqlRunner } from './SqlRunner.js';

interface Row { TicketId: string; At: Date; ById: string; ByName: string; Field: string; FromValue: string | null; ToValue: string | null; Note: string | null }

// Verlauf in board.History.
export class MssqlHistoryRepository implements BoardHistoryRepository {
  constructor(private readonly sql: SqlRunner) {}

  async list(ticketId: string): Promise<BoardHistoryEntry[]> {
    const { rows } = await this.sql.query<Row>(
      'SELECT TicketId, At, ById, ByName, Field, FromValue, ToValue, Note FROM board.History WHERE TicketId = @ticketId ORDER BY Id',
      { ticketId },
    );
    return rows.map((r) => ({ ticketId: r.TicketId, at: SqlRunner.iso(r.At)!, byId: SqlRunner.guid(r.ById), byName: r.ByName, field: r.Field, from: r.FromValue, to: r.ToValue, note: r.Note }));
  }

  async insert(entries: BoardHistoryEntry[]): Promise<void> {
    for (const e of entries) {
      await this.sql.query(
        'INSERT INTO board.History (TicketId, At, ById, ByName, Field, FromValue, ToValue, Note) VALUES (@ticketId, @at, @byId, @byName, @field, @from, @to, @note)',
        { ticketId: e.ticketId, at: SqlRunner.date(e.at), byId: e.byId, byName: e.byName, field: e.field, from: e.from, to: e.to, note: e.note },
      );
    }
  }
}
