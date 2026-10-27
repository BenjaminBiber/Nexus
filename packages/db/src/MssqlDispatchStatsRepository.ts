import { DISPATCH_MARKERS, type BoardComment, type BoardHistoryEntry, type DispatchCommentCounts, type DispatchRunFilter, type DispatchStatsRepository } from '@harness/core';
import { SqlChunks } from './SqlChunks.js';
import { SqlRunner } from './SqlRunner.js';

interface HistoryRow { TicketId: string; At: Date; ById: string; ByName: string; Field: string; FromValue: string | null; ToValue: string | null; Note: string | null }
interface CommentRow { Id: number | string; TicketId: string; AuthorId: string; AuthorName: string; Body: string; CreatedAt: Date; ReadByAgentAt: Date | null }

// Lese-Abfragen der Auswertung über Verlauf und Kommentare (Präfixe per LEFT statt LIKE, „[“ wäre dort ein Platzhalter).
export class MssqlDispatchStatsRepository implements DispatchStatsRepository {
  constructor(private readonly sql: SqlRunner) {}

  async doneTicketIds(filter: DispatchRunFilter): Promise<string[]> {
    const { rows } = await this.sql.query<{ TicketId: string }>(
      `SELECT h.TicketId FROM board.History h JOIN board.Tickets t ON t.Id = h.TicketId
       WHERE h.Field = 'status' AND h.ToValue = N'done' AND t.ProjectId = @projectId${filter.boardId === undefined ? '' : ' AND t.BoardId = @boardId'}
       GROUP BY h.TicketId HAVING MAX(h.At) >= @from AND MAX(h.At) < @to ORDER BY h.TicketId`,
      { projectId: filter.projectId, from: SqlRunner.date(filter.from), to: SqlRunner.date(filter.to), ...(filter.boardId === undefined ? {} : { boardId: filter.boardId }) },
    );
    return rows.map((r) => r.TicketId);
  }

  async statusHistory(ticketIds: string[]): Promise<BoardHistoryEntry[]> {
    const out: Array<BoardHistoryEntry & { id: number }> = [];
    for (const chunk of SqlChunks.of(ticketIds)) {
      const { list, params } = SqlChunks.params('t', chunk);
      const { rows } = await this.sql.query<HistoryRow & { Id: number | string }>(
        `SELECT Id, TicketId, At, ById, ByName, Field, FromValue, ToValue, Note FROM board.History WHERE Field IN ('status', 'blockedReason') AND TicketId IN (${list})`, params,
      );
      out.push(...rows.map((r) => ({
        id: Number(r.Id), ticketId: r.TicketId, at: SqlRunner.iso(r.At)!, byId: SqlRunner.guid(r.ById), byName: r.ByName, field: r.Field,
        from: r.FromValue, to: r.ToValue, note: r.Note,
      })));
    }
    return out.sort((a, b) => a.id - b.id).map(({ id: _id, ...entry }) => entry);
  }

  async dispatchCommentCounts(ticketIds: string[]): Promise<Record<string, DispatchCommentCounts>> {
    const result: Record<string, DispatchCommentCounts> = {};
    for (const chunk of SqlChunks.of(ticketIds)) {
      const { list, params } = SqlChunks.params('t', chunk);
      const { rows } = await this.sql.query<{ TicketId: string; Starts: number; Aborts: number }>(
        `SELECT TicketId,
           SUM(CASE WHEN LEFT(Body, LEN(@start)) = @start THEN 1 ELSE 0 END) AS Starts,
           SUM(CASE WHEN LEFT(Body, LEN(@abort)) = @abort THEN 1 ELSE 0 END) AS Aborts
         FROM board.Comments
         WHERE TicketId IN (${list}) AND (LEFT(Body, LEN(@start)) = @start OR LEFT(Body, LEN(@abort)) = @abort)
         GROUP BY TicketId`,
        { ...params, start: DISPATCH_MARKERS.START, abort: DISPATCH_MARKERS.ABORT },
      );
      for (const r of rows) result[r.TicketId] = { starts: Number(r.Starts), aborts: Number(r.Aborts) };
    }
    return result;
  }

  async dispatchComments(projectId: number): Promise<BoardComment[]> {
    const { rows } = await this.sql.query<CommentRow>(
      `SELECT c.Id, c.TicketId, c.AuthorId, c.AuthorName, c.Body, c.CreatedAt, c.ReadByAgentAt
       FROM board.Comments c JOIN board.Tickets t ON t.Id = c.TicketId
       WHERE t.ProjectId = @projectId AND LEFT(c.Body, 11) = N'[dispatch] ' AND SUBSTRING(c.Body, 11, 1) = N' '
       ORDER BY c.Id`,
      { projectId },
    );
    return rows.map((r) => ({
      id: Number(r.Id), ticketId: r.TicketId, authorId: SqlRunner.guid(r.AuthorId), authorName: r.AuthorName, body: r.Body,
      createdAt: SqlRunner.iso(r.CreatedAt)!, readByAgentAt: SqlRunner.iso(r.ReadByAgentAt),
    }));
  }
}
