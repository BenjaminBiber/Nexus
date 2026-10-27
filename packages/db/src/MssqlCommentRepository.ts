import type { BoardComment, BoardCommentRepository, NewBoardComment } from '@harness/core';
import { SqlChunks } from './SqlChunks.js';
import { SqlRunner, type SqlValue } from './SqlRunner.js';

interface Row { Id: string | number; TicketId: string; AuthorId: string; AuthorName: string; Body: string; CreatedAt: Date; ReadByAgentAt: Date | null }

const COLUMNS = 'Id, TicketId, AuthorId, AuthorName, Body, CreatedAt, ReadByAgentAt';

// Kommentare in board.Comments (nur anhängen; einzig die Lesemarke des Agenten wird nachgetragen).
export class MssqlCommentRepository implements BoardCommentRepository {
  constructor(private readonly sql: SqlRunner) {}

  async list(ticketId: string): Promise<BoardComment[]> {
    const { rows } = await this.sql.query<Row>(`SELECT ${COLUMNS} FROM board.Comments WHERE TicketId = @ticketId ORDER BY Id`, { ticketId });
    return rows.map((r) => this.map(r));
  }

  async insert(c: NewBoardComment): Promise<BoardComment> {
    const readByAgentAt = c.readByAgentAt ?? null;
    const { rows } = await this.sql.query<{ Id: string | number }>(
      `INSERT INTO board.Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt, ReadByAgentAt) OUTPUT inserted.Id
       VALUES (@ticketId, @authorId, @authorName, @body, @createdAt, @readByAgentAt)`,
      {
        ticketId: c.ticketId, authorId: c.authorId, authorName: c.authorName, body: c.body,
        createdAt: SqlRunner.date(c.createdAt), readByAgentAt: SqlRunner.date(readByAgentAt),
      },
    );
    return { ...c, readByAgentAt, id: Number(rows[0].Id) };
  }

  async countByBoard(boardId: number): Promise<Record<string, number>> {
    const { rows } = await this.sql.query<{ TicketId: string; N: number }>(
      'SELECT c.TicketId, COUNT(*) AS N FROM board.Comments c JOIN board.Tickets t ON t.Id = c.TicketId WHERE t.BoardId = @boardId GROUP BY c.TicketId',
      { boardId },
    );
    return Object.fromEntries(rows.map((r) => [r.TicketId, Number(r.N)]));
  }

  async unreadByAgent(ticketIds: string[]): Promise<BoardComment[]> {
    const result: BoardComment[] = [];
    for (const chunk of SqlChunks.of(ticketIds)) {
      const { list, params } = SqlChunks.params('t', chunk);
      const { rows } = await this.sql.query<Row>(`SELECT ${COLUMNS} FROM board.Comments WHERE ReadByAgentAt IS NULL AND TicketId IN (${list})`, params);
      result.push(...rows.map((r) => this.map(r)));
    }
    return result.sort((a, b) => a.id - b.id);
  }

  async markReadByAgent(ids: number[], at: string): Promise<void> {
    for (const chunk of SqlChunks.of(ids)) {
      const { list, params } = SqlChunks.params('c', chunk);
      const values: Record<string, SqlValue> = { ...params, at: SqlRunner.date(at) };
      await this.sql.query(`UPDATE board.Comments SET ReadByAgentAt = @at WHERE ReadByAgentAt IS NULL AND Id IN (${list})`, values);
    }
  }

  // Zeile → Kommentar
  private map(r: Row): BoardComment {
    return {
      id: Number(r.Id), ticketId: r.TicketId, authorId: SqlRunner.guid(r.AuthorId), authorName: r.AuthorName, body: r.Body,
      createdAt: SqlRunner.iso(r.CreatedAt)!, readByAgentAt: SqlRunner.iso(r.ReadByAgentAt),
    };
  }
}
