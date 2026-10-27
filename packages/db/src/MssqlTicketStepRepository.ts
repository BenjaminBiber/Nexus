import type { StepProgress, TicketStep, TicketStepRepository } from '@harness/core';
import { SqlChunks } from './SqlChunks.js';
import type { SqlRunner } from './SqlRunner.js';

interface Row { Text: string; Done: boolean; Detail: string | null }

// Arbeitsschritte je Ticket in board.TicketSteps (Position ab 0).
export class MssqlTicketStepRepository implements TicketStepRepository {
  constructor(private readonly sql: SqlRunner) {}

  async list(ticketId: string): Promise<TicketStep[]> {
    const { rows } = await this.sql.query<Row>('SELECT Text, Done, Detail FROM board.TicketSteps WHERE TicketId = @ticketId ORDER BY Position', { ticketId });
    return rows.map((r) => (r.Detail === null ? { text: r.Text, done: Boolean(r.Done) } : { text: r.Text, done: Boolean(r.Done), detail: r.Detail }));
  }

  async replace(ticketId: string, steps: TicketStep[]): Promise<void> {
    await this.sql.query('DELETE FROM board.TicketSteps WHERE TicketId = @ticketId', { ticketId });
    for (const [position, step] of steps.entries()) {
      await this.sql.query(
        'INSERT INTO board.TicketSteps (TicketId, Position, Text, Done, Detail) VALUES (@ticketId, @position, @text, @done, @detail)',
        { ticketId, position, text: step.text, done: step.done, detail: step.detail ?? null },
      );
    }
  }

  async setDone(ticketId: string, position: number, done: boolean): Promise<boolean> {
    const { affected } = await this.sql.query('UPDATE board.TicketSteps SET Done = @done WHERE TicketId = @ticketId AND Position = @position', { ticketId, position, done });
    return affected > 0;
  }

  async progress(ticketIds: string[]): Promise<Record<string, StepProgress>> {
    const result: Record<string, StepProgress> = {};
    for (const chunk of SqlChunks.of(ticketIds)) {
      const { list, params } = SqlChunks.params('t', chunk);
      const { rows } = await this.sql.query<{ TicketId: string; Done: number; Total: number }>(
        `SELECT TicketId, SUM(CASE WHEN Done = 1 THEN 1 ELSE 0 END) AS Done, COUNT(*) AS Total FROM board.TicketSteps WHERE TicketId IN (${list}) GROUP BY TicketId`,
        params,
      );
      for (const r of rows) result[r.TicketId] = { done: Number(r.Done), total: Number(r.Total) };
    }
    return result;
  }
}
