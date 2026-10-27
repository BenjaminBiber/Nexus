import type { BoardNote, BoardNoteRepository, NoteFilter, NoteState } from '@harness/core';
import { SqliteRunner } from './SqliteRunner.js';

interface Row {
  Id: number; BoardId: number; AuthorId: string; AuthorName: string; Title: string; Body: string; Source: string | null;
  Labels: string; SeenCount: number; SeenIn: string; State: NoteState; TicketId: string | null; Reason: string | null; CreatedAt: string;
}

const COLUMNS = 'Id, BoardId, AuthorId, AuthorName, Title, Body, Source, Labels, SeenCount, SeenIn, State, TicketId, Reason, CreatedAt';

// Notizen in Notes; Labels als kommagetrennter Text (Labels enthalten nie Kommas).
export class SqliteNoteRepository implements BoardNoteRepository {
  constructor(private readonly sql: SqliteRunner) {}

  // Leere Statusliste ergibt keine Notizen; ohne Statusliste wird nicht nach Status gefiltert
  async list(filter: NoteFilter): Promise<BoardNote[]> {
    if (filter.states && !filter.states.length) return [];
    const params: Record<string, string | number | boolean | Date | null> = { boardId: filter.boardId, source: filter.source ?? null };
    const states = filter.states ?? [];
    states.forEach((s, i) => { params[`s${i}`] = s; });
    const stateClause = states.length ? `AND State IN (${states.map((_, i) => `@s${i}`).join(', ')})` : '';
    const { rows } = await this.sql.query<Row>(
      `SELECT ${COLUMNS} FROM Notes WHERE BoardId = @boardId ${stateClause} AND (@source IS NULL OR Source = @source) ORDER BY Id`,
      params,
    );
    return rows.map((r) => this.map(r));
  }

  // forUpdate entfällt: BEGIN IMMEDIATE sperrt die ganze Datei
  async byId(id: number): Promise<BoardNote | null> {
    const { rows } = await this.sql.query<Row>(`SELECT ${COLUMNS} FROM Notes WHERE Id = @id`, { id });
    return rows[0] ? this.map(rows[0]) : null;
  }

  async insert(n: Omit<BoardNote, 'id'>): Promise<BoardNote> {
    const { rows } = await this.sql.query<{ Id: number }>(
      `INSERT INTO Notes (BoardId, AuthorId, AuthorName, Title, Body, Source, Labels, SeenCount, SeenIn, State, TicketId, Reason, CreatedAt)
       VALUES (@boardId, @authorId, @authorName, @title, @body, @source, @labels, @seenCount, @seenIn, @state, @ticketId, @reason, @createdAt) RETURNING Id`,
      { boardId: n.boardId, authorId: n.authorId.toLowerCase(), authorName: n.authorName, title: n.title, body: n.body, source: n.source, labels: n.labels.join(','), seenCount: n.seenCount, seenIn: n.seenIn.join(','), state: n.state, ticketId: n.ticketId, reason: n.reason, createdAt: new Date(n.createdAt) },
    );
    return { ...n, id: rows[0].Id };
  }

  async update(n: BoardNote): Promise<void> {
    await this.sql.query(
      'UPDATE Notes SET Title = @title, Body = @body, Labels = @labels, SeenCount = @seenCount, SeenIn = @seenIn, State = @state, TicketId = @ticketId, Reason = @reason WHERE Id = @id',
      { id: n.id, title: n.title, body: n.body, labels: n.labels.join(','), seenCount: n.seenCount, seenIn: n.seenIn.join(','), state: n.state, ticketId: n.ticketId, reason: n.reason },
    );
  }

  private map(r: Row): BoardNote {
    return {
      id: r.Id, boardId: r.BoardId, authorId: SqliteRunner.guid(r.AuthorId), authorName: r.AuthorName, title: r.Title, body: r.Body, source: r.Source,
      labels: r.Labels ? r.Labels.split(',').filter(Boolean) : [], seenCount: r.SeenCount, seenIn: r.SeenIn ? r.SeenIn.split(',').filter(Boolean) : [], state: r.State, ticketId: r.TicketId, reason: r.Reason,
      createdAt: SqliteRunner.iso(r.CreatedAt)!,
    };
  }
}
