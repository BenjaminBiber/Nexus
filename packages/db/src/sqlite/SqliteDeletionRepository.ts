import type { BoardDeletion, BoardDeletionRepository, BoardTicketStatus } from '@harness/core';
import { SqliteRunner } from './SqliteRunner.js';

interface Row { At: string; ById: string; ByName: string; TicketId: string; TicketTitle: string; FromStatus: BoardTicketStatus | null }

// Protokoll gelöschter Tickets in Deletions.
export class SqliteDeletionRepository implements BoardDeletionRepository {
  constructor(private readonly sql: SqliteRunner) {}

  async insert(d: BoardDeletion): Promise<void> {
    await this.sql.query(
      'INSERT INTO Deletions (At, ById, ByName, TicketId, TicketTitle, FromStatus) VALUES (@at, @byId, @byName, @ticketId, @ticketTitle, @fromStatus)',
      { at: new Date(d.at), byId: d.byId.toLowerCase(), byName: d.byName, ticketId: d.ticketId, ticketTitle: d.ticketTitle, fromStatus: d.fromStatus },
    );
  }

  async list(): Promise<BoardDeletion[]> {
    const { rows } = await this.sql.query<Row>('SELECT At, ById, ByName, TicketId, TicketTitle, FromStatus FROM Deletions ORDER BY Id');
    return rows.map((r) => ({ at: SqliteRunner.iso(r.At)!, byId: SqliteRunner.guid(r.ById), byName: r.ByName, ticketId: r.TicketId, ticketTitle: r.TicketTitle, fromStatus: r.FromStatus }));
  }
}
