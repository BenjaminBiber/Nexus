import { DISPATCH_MARKERS, type BoardComment, type BoardHistoryEntry, type DispatchCommentCounts, type DispatchRunFilter, type DispatchStatsRepository } from '@harness/core';
import { SqlChunks } from '../SqlChunks.js';
import { SqliteRunner } from './SqliteRunner.js';

interface HistoryRow { TicketId: string; At: string; ById: string; ByName: string; Field: string; FromValue: string | null; ToValue: string | null; Note: string | null }
interface CommentRow { Id: number; TicketId: string; AuthorId: string; AuthorName: string; Body: string; CreatedAt: string; ReadByAgentAt: string | null }

// Lese-Abfragen der Auswertung über Verlauf und Kommentare (Präfixe per substr/length statt LEFT/LEN; SQLite vergleicht Text exakt).
export class SqliteDispatchStatsRepository implements DispatchStatsRepository {
  constructor(private readonly sql: SqliteRunner) {}

  async doneTicketIds(filter: DispatchRunFilter): Promise<string[]> {
    const { rows } = await this.sql.query<{ TicketId: string }>(
      `SELECT h.TicketId FROM History h JOIN Tickets t ON t.Id = h.TicketId
       WHERE h.Field = 'status' AND h.ToValue = 'done' AND t.ProjectId = @projectId${filter.boardId === undefined ? '' : ' AND t.BoardId = @boardId'}
       GROUP BY h.TicketId HAVING MAX(h.At) >= @from AND MAX(h.At) < @to ORDER BY h.TicketId`,
      { projectId: filter.projectId, from: new Date(filter.from), to: new Date(filter.to), ...(filter.boardId === undefined ? {} : { boardId: filter.boardId }) },
    );
    return rows.map((r) => r.TicketId);
  }

  async statusHistory(ticketIds: string[]): Promise<BoardHistoryEntry[]> {
    const out: Array<BoardHistoryEntry & { id: number }> = [];
    for (const chunk of SqlChunks.of(ticketIds)) {
      const { list, params } = SqlChunks.params('t', chunk);
      const { rows } = await this.sql.query<HistoryRow & { Id: number }>(
        `SELECT Id, TicketId, At, ById, ByName, Field, FromValue, ToValue, Note FROM History WHERE Field IN ('status', 'blockedReason') AND TicketId IN (${list})`, params,
      );
      out.push(...rows.map((r) => ({
        id: Number(r.Id), ticketId: r.TicketId, at: SqliteRunner.iso(r.At)!, byId: SqliteRunner.guid(r.ById), byName: r.ByName, field: r.Field,
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
           SUM(CASE WHEN substr(Body, 1, length(@start)) = @start THEN 1 ELSE 0 END) AS Starts,
           SUM(CASE WHEN substr(Body, 1, length(@abort)) = @abort THEN 1 ELSE 0 END) AS Aborts
         FROM Comments
         WHERE TicketId IN (${list}) AND (substr(Body, 1, length(@start)) = @start OR substr(Body, 1, length(@abort)) = @abort)
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
       FROM Comments c JOIN Tickets t ON t.Id = c.TicketId
       WHERE t.ProjectId = @projectId AND substr(c.Body, 1, 11) = '[dispatch] '
       ORDER BY c.Id`,
      { projectId },
    );
    return rows.map((r) => ({
      id: Number(r.Id), ticketId: r.TicketId, authorId: SqliteRunner.guid(r.AuthorId), authorName: r.AuthorName, body: r.Body,
      createdAt: SqliteRunner.iso(r.CreatedAt)!, readByAgentAt: SqliteRunner.iso(r.ReadByAgentAt),
    }));
  }
}
