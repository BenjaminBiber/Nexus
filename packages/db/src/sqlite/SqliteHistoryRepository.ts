import type { BoardHistoryEntry, BoardHistoryRepository } from '@harness/core';
import { SqliteRunner } from './SqliteRunner.js';

interface Row { TicketId: string; At: string; ById: string; ByName: string; Field: string; FromValue: string | null; ToValue: string | null; Note: string | null }

// Verlauf in History.
export class SqliteHistoryRepository implements BoardHistoryRepository {
  constructor(private readonly sql: SqliteRunner) {}

  async list(ticketId: string): Promise<BoardHistoryEntry[]> {
    const { rows } = await this.sql.query<Row>(
      'SELECT TicketId, At, ById, ByName, Field, FromValue, ToValue, Note FROM History WHERE TicketId = @ticketId ORDER BY Id',
      { ticketId },
    );
    return rows.map((r) => ({ ticketId: r.TicketId, at: SqliteRunner.iso(r.At)!, byId: SqliteRunner.guid(r.ById), byName: r.ByName, field: r.Field, from: r.FromValue, to: r.ToValue, note: r.Note }));
  }

  async insert(entries: BoardHistoryEntry[]): Promise<void> {
    for (const e of entries) {
      await this.sql.query(
        'INSERT INTO History (TicketId, At, ById, ByName, Field, FromValue, ToValue, Note) VALUES (@ticketId, @at, @byId, @byName, @field, @from, @to, @note)',
        { ticketId: e.ticketId, at: new Date(e.at), byId: e.byId.toLowerCase(), byName: e.byName, field: e.field, from: e.from, to: e.to, note: e.note },
      );
    }
  }
}
