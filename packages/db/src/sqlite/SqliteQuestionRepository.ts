import {
  HarnessError, type AuthorKind, type BoardQuestion, type BoardQuestionFilter, type BoardQuestionRepository, type QuestionState,
} from '@harness/core';
import { SqliteRunner, type SqliteValue } from './SqliteRunner.js';

interface Row {
  Id: string; ProjectId: number; Number: number; Title: string; Description: string; State: QuestionState; Occasion: string | null;
  OriginKind: AuthorKind; OriginName: string; OriginSession: string | null;
  DecisionOutcome: 'answered' | 'discarded' | null; DecisionOption: string | null; DecisionText: string | null; DecisionReason: string | null;
  DecidedById: string | null; DecidedByName: string | null; DecidedAt: string | null;
  CreatedAt: string; CreatedById: string; UpdatedAt: string; Rev: number;
}

const COLUMNS = `q.Id, q.ProjectId, q.Number, q.Title, q.Description, q.State, q.Occasion, q.OriginKind, q.OriginName, q.OriginSession,
  q.DecisionOutcome, q.DecisionOption, q.DecisionText, q.DecisionReason, q.DecidedById, q.DecidedByName, q.DecidedAt,
  q.CreatedAt, q.CreatedById, q.UpdatedAt, q.Rev`;

// Fragen in Questions samt Tags, betroffenen Tickets und Abhängigkeiten.
export class SqliteQuestionRepository implements BoardQuestionRepository {
  constructor(private readonly sql: SqliteRunner) {}

  // forUpdate ist bei SQLite ein No-op (Schreibtransaktionen laufen mit BEGIN IMMEDIATE)
  async byId(id: string): Promise<BoardQuestion | null> {
    const found = await this.load('WHERE q.Id = @id', { id });
    return found[0] ?? null;
  }

  // Leere Listen im Filter ergeben keine Fragen
  async list(filter: BoardQuestionFilter = {}): Promise<BoardQuestion[]> {
    const where: string[] = [];
    const params: Record<string, SqliteValue> = {};
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
      where.push(`q.Id IN (SELECT QuestionId FROM QuestionTickets WHERE TicketId IN (${this.bind(params, 't', filter.ticketIds)}))`);
    }
    return this.load(where.length ? `WHERE ${where.join(' AND ')}` : '', params);
  }

  async insert(q: BoardQuestion): Promise<void> {
    await this.sql.query(
      `INSERT INTO Questions (Id, ProjectId, Number, Title, Description, State, Occasion, OriginKind, OriginName, OriginSession,
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
      `UPDATE Questions SET Title = @title, Description = @description, State = @state, Occasion = @occasion,
         DecisionOutcome = @decisionOutcome, DecisionOption = @decisionOption, DecisionText = @decisionText,
         DecisionReason = @decisionReason, DecidedById = @decidedById, DecidedByName = @decidedByName, DecidedAt = @decidedAt,
         UpdatedAt = @updatedAt, Rev = @rev
       WHERE Id = @id AND Rev = @previousRev`,
      { ...this.params(q), previousRev: q.rev - 1 },
    );
    if (affected === 0) throw new HarnessError('conflict', `Frage ${q.id} wurde inzwischen geändert. Bitte neu lesen.`);
    // Je Aufruf genau eine Anweisung (der MSSQL-Batch ist hier in drei Aufrufe zerlegt)
    await this.sql.query('DELETE FROM QuestionTags WHERE QuestionId = @id', { id: q.id });
    await this.sql.query('DELETE FROM QuestionTickets WHERE QuestionId = @id', { id: q.id });
    await this.sql.query('DELETE FROM QuestionDependencies WHERE QuestionId = @id', { id: q.id });
    await this.writeChildren(q);
  }

  // Liest die Fragen, die das Ticket betreffen oder davon abhängen (ohne Sperre)
  async idsReferencingTicket(ticketId: string): Promise<string[]> {
    const { rows } = await this.sql.query<{ QuestionId: string }>(
      'SELECT QuestionId FROM QuestionTickets WHERE TicketId = @ticketId UNION SELECT QuestionId FROM QuestionDependencies WHERE DependsOnId = @ticketId',
      { ticketId },
    );
    return [...new Set(rows.map((r) => r.QuestionId))].sort();
  }

  // Entfernt Bezüge und Abhängigkeiten auf das Ticket; die Fragen selbst bleiben unverändert (Rev erhöht der Dienst)
  async removeTicket(ticketId: string): Promise<string[]> {
    const tickets = await this.sql.query<{ QuestionId: string }>('DELETE FROM QuestionTickets WHERE TicketId = @ticketId RETURNING QuestionId', { ticketId });
    const deps = await this.sql.query<{ QuestionId: string }>('DELETE FROM QuestionDependencies WHERE DependsOnId = @ticketId RETURNING QuestionId', { ticketId });
    return [...new Set([...tickets.rows, ...deps.rows].map((r) => r.QuestionId))].sort();
  }

  // Lädt Fragen zu einer WHERE-Klausel und ergänzt Tags, Tickets und Abhängigkeiten
  private async load(where: string, params: Record<string, SqliteValue>): Promise<BoardQuestion[]> {
    const { rows } = await this.sql.query<Row>(`SELECT ${COLUMNS} FROM Questions q ${where} ORDER BY q.ProjectId, q.Number`, params);
    if (!rows.length) return [];
    const ids = `SELECT q.Id FROM Questions q ${where}`;
    const tags = await this.sql.query<{ QuestionId: string; Tag: string }>(`SELECT QuestionId, Tag FROM QuestionTags WHERE QuestionId IN (${ids}) ORDER BY QuestionId, Position`, params);
    const tickets = await this.sql.query<{ QuestionId: string; TicketId: string }>(`SELECT QuestionId, TicketId FROM QuestionTickets WHERE QuestionId IN (${ids})`, params);
    const deps = await this.sql.query<{ QuestionId: string; DependsOnId: string }>(`SELECT QuestionId, DependsOnId FROM QuestionDependencies WHERE QuestionId IN (${ids}) ORDER BY QuestionId, Position`, params);
    return rows.map((r) => ({
      id: r.Id, projectId: Number(r.ProjectId), number: Number(r.Number), title: r.Title, description: r.Description, state: r.State, occasion: r.Occasion,
      originKind: r.OriginKind, originName: r.OriginName, originSession: r.OriginSession,
      tags: tags.rows.filter((t) => t.QuestionId === r.Id).map((t) => t.Tag),
      ticketIds: tickets.rows.filter((t) => t.QuestionId === r.Id).map((t) => t.TicketId).sort(),
      dependsOn: deps.rows.filter((d) => d.QuestionId === r.Id).map((d) => d.DependsOnId),
      decision: r.DecisionOutcome
        ? {
            outcome: r.DecisionOutcome, option: r.DecisionOption, text: r.DecisionText, reason: r.DecisionReason ?? '',
            byId: SqliteRunner.guid(r.DecidedById ?? ''), byName: r.DecidedByName ?? '', at: SqliteRunner.iso(r.DecidedAt)!,
          }
        : null,
      createdAt: SqliteRunner.iso(r.CreatedAt)!, createdById: SqliteRunner.guid(r.CreatedById), updatedAt: SqliteRunner.iso(r.UpdatedAt)!, rev: Number(r.Rev),
    }));
  }

  // Schreibt Tags (mit Position), betroffene Tickets und Abhängigkeiten (mit Position)
  private async writeChildren(q: BoardQuestion): Promise<void> {
    for (const [position, tag] of q.tags.entries()) {
      await this.sql.query('INSERT INTO QuestionTags (QuestionId, Position, Tag) VALUES (@id, @position, @tag)', { id: q.id, position, tag });
    }
    for (const ticketId of q.ticketIds) {
      await this.sql.query('INSERT INTO QuestionTickets (QuestionId, TicketId) VALUES (@id, @ticketId)', { id: q.id, ticketId });
    }
    for (const [position, dep] of q.dependsOn.entries()) {
      await this.sql.query('INSERT INTO QuestionDependencies (QuestionId, Position, DependsOnId) VALUES (@id, @position, @dep)', { id: q.id, position, dep });
    }
  }

  // Bindet eine Liste als @präfix0, @präfix1, … und liefert die Platzhalter
  private bind(params: Record<string, SqliteValue>, prefix: string, values: SqliteValue[]): string {
    values.forEach((value, i) => {
      params[`${prefix}${i}`] = value;
    });
    return values.map((_, i) => `@${prefix}${i}`).join(', ');
  }

  // Parameter einer Frage für INSERT/UPDATE (GUIDs klein, Datumswerte als Date → ISO-Text)
  private params(q: BoardQuestion): Record<string, SqliteValue> {
    const d = q.decision;
    return {
      id: q.id, projectId: q.projectId, number: q.number, title: q.title, description: q.description, state: q.state, occasion: q.occasion,
      originKind: q.originKind, originName: q.originName, originSession: q.originSession,
      decisionOutcome: d?.outcome ?? null, decisionOption: d?.option ?? null, decisionText: d?.text ?? null, decisionReason: d?.reason ?? null,
      decidedById: d ? d.byId.toLowerCase() : null, decidedByName: d?.byName ?? null, decidedAt: SqliteRunner.date(d?.at ?? null),
      createdAt: SqliteRunner.date(q.createdAt), createdById: q.createdById.toLowerCase(), updatedAt: SqliteRunner.date(q.updatedAt), rev: q.rev,
    };
  }
}
