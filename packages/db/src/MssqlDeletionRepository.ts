import type { BoardDeletion, BoardDeletionRepository, BoardTicketStatus } from '@harness/core';
import { SqlRunner } from './SqlRunner.js';

interface Row { At: Date; ById: string; ByName: string; TicketId: string; TicketTitle: string; FromStatus: BoardTicketStatus | null }

// Protokoll gelöschter Tickets in board.Deletions.
export class MssqlDeletionRepository implements BoardDeletionRepository {
  constructor(private readonly sql: SqlRunner) {}

  async insert(d: BoardDeletion): Promise<void> {
    await this.sql.query(
      'INSERT INTO board.Deletions (At, ById, ByName, TicketId, TicketTitle, FromStatus) VALUES (@at, @byId, @byName, @ticketId, @ticketTitle, @fromStatus)',
      { at: SqlRunner.date(d.at), byId: d.byId, byName: d.byName, ticketId: d.ticketId, ticketTitle: d.ticketTitle, fromStatus: d.fromStatus },
    );
  }

  async list(): Promise<BoardDeletion[]> {
    const { rows } = await this.sql.query<Row>('SELECT At, ById, ByName, TicketId, TicketTitle, FromStatus FROM board.Deletions ORDER BY Id');
    return rows.map((r) => ({ at: SqlRunner.iso(r.At)!, byId: SqlRunner.guid(r.ById), byName: r.ByName, ticketId: r.TicketId, ticketTitle: r.TicketTitle, fromStatus: r.FromStatus }));
  }
}
