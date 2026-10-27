import {
  HarnessError, type AuthorKind, type BoardQuestion, type BoardQuestionFilter, type BoardQuestionRepository, type QuestionState,
} from '@harness/core';
import { SqlRunner, type SqlValue } from './SqlRunner.js';

interface Row {
  Id: string; ProjectId: number; Number: number; Title: string; Description: string; State: QuestionState; Occasion: string | null;
  OriginKind: AuthorKind; OriginName: string; OriginSession: string | null;
  DecisionOutcome: 'answered' | 'discarded' | null; DecisionOption: string | null; DecisionText: string | null; DecisionReason: string | null;
  DecidedById: string | null; DecidedByName: string | null; DecidedAt: Date | null;
  CreatedAt: Date; CreatedById: string; UpdatedAt: Date; Rev: number;
}

const COLUMNS = `q.Id, q.ProjectId, q.Number, q.Title, q.Description, q.State, q.Occasion, q.OriginKind, q.OriginName, q.OriginSession,
  q.DecisionOutcome, q.DecisionOption, q.DecisionText, q.DecisionReason, q.DecidedById, q.DecidedByName, q.DecidedAt,
  q.CreatedAt, q.CreatedById, q.UpdatedAt, q.Rev`;

// Fragen in board.Questions samt Tags, betroffenen Tickets und Abhängigkeiten.
export class MssqlQuestionRepository implements BoardQuestionRepository {
  constructor(private readonly sql: SqlRunner) {}

  // forUpdate sperrt die Zeile bis zum Ende der Transaktion
  async byId(id: string, forUpdate = false): Promise<BoardQuestion | null> {
    const hint = forUpdate ? 'WITH (UPDLOCK, ROWLOCK)' : '';
    const found = await this.load(`FROM board.Questions q ${hint} WHERE q.Id = @id`, { id });
    return found[0] ?? null;
  }

  // Leere Listen im Filter ergeben keine Fragen
  async list(filter: BoardQuestionFilter = {}): Promise<BoardQuestion[]> {
    const where: string[] = [];
    const params: Record<string, SqlValue> = {};
    if (filter.projectId !== undefined) {
      where.push('q.ProjectId = @projectId');
      params.projectId = filter.projectId;
    }
    if (filter.ids) {
      if (!filter.ids.length) return [];
      where.push(`q.Id IN (${this.bind(params, 'i', filter.ids)})`);
    }
    if (filter.states) {
      if (!filter.states.length) return [];
      where.push(`q.State IN (${this.bind(params, 's', filter.states)})`);
    }
    if (filter.ticketIds) {
      if (!filter.ticketIds.length) return [];
      where.push(`q.Id IN (SELECT QuestionId FROM board.QuestionTickets WHERE TicketId IN (${this.bind(params, 't', filter.ticketIds)}))`);
    }
    return this.load(`FROM board.Questions q ${where.length ? `WHERE ${where.join(' AND ')}` : ''}`, params);
  }

  async insert(q: BoardQuestion): Promise<void> {
    await this.sql.query(
      `INSERT INTO board.Questions (Id, ProjectId, Number, Title, Description, State, Occasion, OriginKind, OriginName, OriginSession,
         DecisionOutcome, DecisionOption, DecisionText, DecisionReason, DecidedById, DecidedByName, DecidedAt,
         CreatedAt, CreatedById, UpdatedAt, Rev)
       VALUES (@id, @projectId, @number, @title, @description, @state, @occasion, @originKind, @originName, @originSession,
         @decisionOutcome, @decisionOption, @decisionText, @decisionReason, @decidedById, @decidedByName, @decidedAt,
         @createdAt, @createdById, @updatedAt, @rev)`,
      this.params(q),
    );
    await this.writeChildren(q);
  }

  // Schreibt nur, wenn in der DB noch die Vorgängerversion steht
  async update(q: BoardQuestion): Promise<void> {
    const { affected } = await this.sql.query(
      `UPDATE board.Questions SET Title = @title, Description = @description, State = @state, Occasion = @occasion,
         DecisionOutcome = @decisionOutcome, DecisionOption = @decisionOption, DecisionText = @decisionText,
         DecisionReason = @decisionReason, DecidedById = @decidedById, DecidedByName = @decidedByName, DecidedAt = @decidedAt,
         UpdatedAt = @updatedAt, Rev = @rev
       WHERE Id = @id AND Rev = @previousRev`,
      { ...this.params(q), previousRev: q.rev - 1 },
    );
    if (affected === 0) throw new HarnessError('conflict', `Frage ${q.id} wurde inzwischen geändert. Bitte neu lesen.`);
    await this.sql.query(
      'DELETE FROM board.QuestionTags WHERE QuestionId = @id; DELETE FROM board.QuestionTickets WHERE QuestionId = @id; DELETE FROM board.QuestionDependencies WHERE QuestionId = @id;',
      { id: q.id },
    );
    await this.writeChildren(q);
  }

  // Liest die Fragen, die das Ticket betreffen oder davon abhängen (ohne Sperre)
  async idsReferencingTicket(ticketId: string): Promise<string[]> {
    const { rows } = await this.sql.query<{ QuestionId: string }>(
      'SELECT QuestionId FROM board.QuestionTickets WHERE TicketId = @ticketId UNION SELECT QuestionId FROM board.QuestionDependencies WHERE DependsOnId = @ticketId',
      { ticketId },
    );
    return [...new Set(rows.map((r) => r.QuestionId))].sort();
  }

  // Entfernt Bezüge und Abhängigkeiten auf das Ticket; die Fragen selbst bleiben unverändert (Rev erhöht der Dienst)
  async removeTicket(ticketId: string): Promise<string[]> {
    const tickets = await this.sql.query<{ QuestionId: string }>('DELETE FROM board.QuestionTickets OUTPUT deleted.QuestionId WHERE TicketId = @ticketId', { ticketId });
    const deps = await this.sql.query<{ QuestionId: string }>('DELETE FROM board.QuestionDependencies OUTPUT deleted.QuestionId WHERE DependsOnId = @ticketId', { ticketId });
    return [...new Set([...tickets.rows, ...deps.rows].map((r) => r.QuestionId))].sort();
  }

  // Lädt Fragen zu einer FROM/WHERE-Klausel und ergänzt Tags, Tickets und Abhängigkeiten
  private async load(from: string, params: Record<string, SqlValue>): Promise<BoardQuestion[]> {
    const { rows } = await this.sql.query<Row>(`SELECT ${COLUMNS} ${from} ORDER BY q.ProjectId, q.Number`, params);
    if (!rows.length) return [];
    const ids = `SELECT q.Id ${from.replace(/WITH \(UPDLOCK, ROWLOCK\)/, '')}`;
    const tags = await this.sql.query<{ QuestionId: string; Tag: string }>(`SELECT QuestionId, Tag FROM board.QuestionTags WHERE QuestionId IN (${ids}) ORDER BY QuestionId, Position`, params);
    const tickets = await this.sql.query<{ QuestionId: string; TicketId: string }>(`SELECT QuestionId, TicketId FROM board.QuestionTickets WHERE QuestionId IN (${ids})`, params);
    const deps = await this.sql.query<{ QuestionId: string; DependsOnId: string }>(`SELECT QuestionId, DependsOnId FROM board.QuestionDependencies WHERE QuestionId IN (${ids}) ORDER BY QuestionId, Position`, params);
    return rows.map((r) => ({
      id: r.Id, projectId: r.ProjectId, number: r.Number, title: r.Title, description: r.Description, state: r.State, occasion: r.Occasion,
      originKind: r.OriginKind, originName: r.OriginName, originSession: r.OriginSession,
      tags: tags.rows.filter((t) => t.QuestionId === r.Id).map((t) => t.Tag),
      ticketIds: tickets.rows.filter((t) => t.QuestionId === r.Id).map((t) => t.TicketId).sort(),
      dependsOn: deps.rows.filter((d) => d.QuestionId === r.Id).map((d) => d.DependsOnId),
      decision: r.DecisionOutcome
        ? {
            outcome: r.DecisionOutcome, option: r.DecisionOption, text: r.DecisionText, reason: r.DecisionReason ?? '',
            byId: SqlRunner.guid(r.DecidedById ?? ''), byName: r.DecidedByName ?? '', at: SqlRunner.iso(r.DecidedAt)!,
          }
        : null,
      createdAt: SqlRunner.iso(r.CreatedAt)!, createdById: SqlRunner.guid(r.CreatedById), updatedAt: SqlRunner.iso(r.UpdatedAt)!, rev: r.Rev,
    }));
  }

  // Schreibt Tags (mit Position), betroffene Tickets und Abhängigkeiten (mit Position)
  private async writeChildren(q: BoardQuestion): Promise<void> {
    for (const [position, tag] of q.tags.entries()) {
      await this.sql.query('INSERT INTO board.QuestionTags (QuestionId, Position, Tag) VALUES (@id, @position, @tag)', { id: q.id, position, tag });
    }
    for (const ticketId of q.ticketIds) {
      await this.sql.query('INSERT INTO board.QuestionTickets (QuestionId, TicketId) VALUES (@id, @ticketId)', { id: q.id, ticketId });
    }
    for (const [position, dep] of q.dependsOn.entries()) {
      await this.sql.query('INSERT INTO board.QuestionDependencies (QuestionId, Position, DependsOnId) VALUES (@id, @position, @dep)', { id: q.id, position, dep });
    }
  }

  // Bindet eine Liste als @präfix0, @präfix1, … und liefert die Platzhalter
  private bind(params: Record<string, SqlValue>, prefix: string, values: SqlValue[]): string {
    values.forEach((value, i) => {
      params[`${prefix}${i}`] = value;
    });
    return values.map((_, i) => `@${prefix}${i}`).join(', ');
  }

  // Parameter einer Frage für INSERT/UPDATE
  private params(q: BoardQuestion): Record<string, SqlValue> {
    const d = q.decision;
    return {
      id: q.id, projectId: q.projectId, number: q.number, title: q.title, description: q.description, state: q.state, occasion: q.occasion,
      originKind: q.originKind, originName: q.originName, originSession: q.originSession,
      decisionOutcome: d?.outcome ?? null, decisionOption: d?.option ?? null, decisionText: d?.text ?? null, decisionReason: d?.reason ?? null,
      decidedById: d?.byId ?? null, decidedByName: d?.byName ?? null, decidedAt: SqlRunner.date(d?.at ?? null),
      createdAt: SqlRunner.date(q.createdAt), createdById: q.createdById, updatedAt: SqlRunner.date(q.updatedAt), rev: q.rev,
    };
  }
}
