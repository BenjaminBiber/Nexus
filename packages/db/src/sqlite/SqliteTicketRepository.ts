import { HarnessError, type BoardTicket, type BoardTicketFilter, type BoardTicketRepository, type BoardTicketStatus, type Priority } from '@harness/core';
import { RowGroups } from '../RowGroups.js';
import { SqliteRunner, type SqliteValue } from './SqliteRunner.js';

interface Row {
  Id: string; ProjectId: number; BoardId: number; Number: number; Title: string; Description: string;
  Status: BoardTicketStatus; Priority: Priority; SortOrder: number; Assignee: string | null; NeedsPlanning: number;
  Acceptance: string; Plan: string; Handbook: string; Diagram: string; BlockedReason: string | null;
  PlannedAt: string | null; PlannedBy: string | null; PlanningClaimBy: string | null; PlanningClaimAt: string | null;
  CreatedAt: string; CreatedById: string; CreatedByName: string; UpdatedAt: string; Rev: number;
  Spec: string | null; SpecTask: string | null; Branch: string | null; Worktree: string | null; TargetBranch: string | null; Touches: string; Consumes: string; Produces: string;
}

const COLUMNS = `t.Id, t.ProjectId, t.BoardId, t.Number, t.Title, t.Description, t.Status, t.Priority, t.SortOrder, t.Assignee,
  t.NeedsPlanning, t.Acceptance, t."Plan", t.Handbook, t.Diagram, t.BlockedReason, t.PlannedAt, t.PlannedBy,
  t.PlanningClaimBy, t.PlanningClaimAt, t.CreatedAt, t.CreatedById, t.CreatedByName, t.UpdatedAt, t.Rev,
  t.Spec, t.SpecTask, t.Branch, t.Worktree, t.TargetBranch, t.Touches, t.Consumes, t.Produces`;

// Wie COLUMNS, aber die langen Texte nur als Kennzeichen (filter.withoutTexts): leer bleibt leer, sonst 'x'
const LIGHT_COLUMNS = ['Description', 'Acceptance', '"Plan"', 'Handbook', 'Diagram']
  .reduce((columns, name) => columns.replace(`t.${name}`, `CASE WHEN length(trim(t.${name})) > 0 THEN 'x' ELSE '' END AS ${name}`), COLUMNS);

// Tickets in Tickets samt Labels (TicketLabels) und Voraussetzungen (TicketDependencies).
export class SqliteTicketRepository implements BoardTicketRepository {
  constructor(private readonly sql: SqliteRunner) {}

  // forUpdate ist hier ein No-op: BEGIN IMMEDIATE sperrt die Datei schon bis zum Ende der Transaktion
  async byId(id: string, _forUpdate = false): Promise<BoardTicket | null> {
    const found = await this.load('FROM Tickets t WHERE t.Id = @id', { id });
    return found[0] ?? null;
  }

  // Sortiert nach Projekt und Nummer, damit die Reihenfolge stabil ist
  async list(filter: BoardTicketFilter = {}): Promise<BoardTicket[]> {
    const where: string[] = [];
    const params: Record<string, SqliteValue> = {};
    if (filter.boardIds) {
      if (!filter.boardIds.length) return [];
      filter.boardIds.forEach((b, i) => { params[`b${i}`] = b; });
      where.push(`t.BoardId IN (${filter.boardIds.map((_, i) => `@b${i}`).join(', ')})`);
    }
    if (filter.ids) {
      if (!filter.ids.length) return [];
      filter.ids.forEach((x, i) => { params[`i${i}`] = x; });
      where.push(`t.Id IN (${filter.ids.map((_, i) => `@i${i}`).join(', ')})`);
    }
    return this.load(`FROM Tickets t ${where.length ? `WHERE ${where.join(' AND ')}` : ''}`, params, filter.withoutTexts);
  }

  async insert(t: BoardTicket): Promise<void> {
    await this.sql.query(
      `INSERT INTO Tickets (Id, ProjectId, BoardId, Number, Title, Description, Status, Priority, SortOrder, Assignee,
         NeedsPlanning, Acceptance, "Plan", Handbook, Diagram, BlockedReason, PlannedAt, PlannedBy, PlanningClaimBy,
         PlanningClaimAt, CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev,
         Spec, SpecTask, Branch, Worktree, TargetBranch, Touches, Consumes, Produces)
       VALUES (@id, @projectId, @boardId, @number, @title, @description, @status, @priority, @sortOrder, @assignee,
         @needsPlanning, @acceptance, @plan, @handbook, @diagram, @blockedReason, @plannedAt, @plannedBy, @planningClaimBy,
         @planningClaimAt, @createdAt, @createdById, @createdByName, @updatedAt, @rev,
         @spec, @specTask, @branch, @worktree, @targetBranch, @touches, @consumes, @produces)`,
      this.params(t),
    );
    await this.writeChildren(t);
  }

  // Schreibt nur, wenn in der DB noch die Vorgängerversion steht; nicht änderbare Spalten stehen nicht im UPDATE
  async update(t: BoardTicket): Promise<void> {
    const { affected } = await this.sql.query(
      `UPDATE Tickets SET BoardId = @boardId, Title = @title, Description = @description, Status = @status,
         Priority = @priority, SortOrder = @sortOrder, Assignee = @assignee, NeedsPlanning = @needsPlanning,
         Acceptance = @acceptance, "Plan" = @plan, Handbook = @handbook, Diagram = @diagram, BlockedReason = @blockedReason,
         PlannedAt = @plannedAt, PlannedBy = @plannedBy, PlanningClaimBy = @planningClaimBy, PlanningClaimAt = @planningClaimAt,
         Spec = @spec, SpecTask = @specTask, Branch = @branch, Worktree = @worktree, TargetBranch = @targetBranch, Touches = @touches, Consumes = @consumes, Produces = @produces,
         UpdatedAt = @updatedAt, Rev = @rev
       WHERE Id = @id AND Rev = @previousRev`,
      { ...this.params(t), previousRev: t.rev - 1 },
    );
    if (affected === 0) throw new HarnessError('conflict', `Ticket ${t.id} wurde inzwischen geändert. Bitte neu lesen.`);
    // Kindzeilen neu schreiben; je Aufruf nur eine Anweisung
    await this.sql.query('DELETE FROM TicketLabels WHERE TicketId = @id', { id: t.id });
    await this.sql.query('DELETE FROM TicketDependencies WHERE TicketId = @id', { id: t.id });
    await this.writeChildren(t);
  }

  // Erst die Verweise anderer Tickets (DependsOnId hat keine Kaskade), dann das Ticket selbst;
  // Labels, eigene Voraussetzungen, Kommentare und Verlauf fallen per Kaskade mit
  async delete(id: string): Promise<void> {
    await this.sql.query('DELETE FROM TicketDependencies WHERE DependsOnId = @id', { id });
    await this.sql.query('DELETE FROM Tickets WHERE Id = @id', { id });
  }

  // Sortiert in JS, damit die Reihenfolge nicht von der DB-Collation abhängt
  async dependents(id: string): Promise<string[]> {
    const { rows } = await this.sql.query<{ TicketId: string }>('SELECT TicketId FROM TicketDependencies WHERE DependsOnId = @id', { id });
    return rows.map((r) => r.TicketId).sort();
  }

  async removeDependencyOn(id: string): Promise<string[]> {
    const { rows } = await this.sql.query<{ TicketId: string }>('DELETE FROM TicketDependencies WHERE DependsOnId = @id RETURNING TicketId', { id });
    return rows.map((r) => r.TicketId).sort();
  }

  async maxSortOrder(boardId: number, status: BoardTicketStatus): Promise<number | null> {
    const { rows } = await this.sql.query<{ M: number | null }>('SELECT MAX(SortOrder) AS M FROM Tickets WHERE BoardId = @boardId AND Status = @status', { boardId, status });
    return rows[0]?.M == null ? null : Number(rows[0].M);
  }

  async openCountByBoard(boardIds: number[]): Promise<Record<number, number>> {
    if (!boardIds.length) return {};
    const params: Record<string, SqliteValue> = {};
    boardIds.forEach((b, i) => { params[`b${i}`] = b; });
    const { rows } = await this.sql.query<{ BoardId: number; N: number }>(
      `SELECT BoardId, COUNT(*) AS N FROM Tickets
       WHERE BoardId IN (${boardIds.map((_, i) => `@b${i}`).join(', ')}) AND Status NOT IN ('done', 'not_needed') GROUP BY BoardId`,
      params,
    );
    return Object.fromEntries(rows.map((r) => [Number(r.BoardId), Number(r.N)]));
  }

  // Lädt Tickets zu einer FROM/WHERE-Klausel und ergänzt Labels und Voraussetzungen
  private async load(from: string, params: Record<string, SqliteValue>, withoutTexts = false): Promise<BoardTicket[]> {
    const { rows } = await this.sql.query<Row>(`SELECT ${withoutTexts ? LIGHT_COLUMNS : COLUMNS} ${from} ORDER BY t.ProjectId, t.Number`, params);
    if (!rows.length) return [];
    const ids = `SELECT t.Id ${from}`;
    const labels = await this.sql.query<{ TicketId: string; Label: string }>(`SELECT TicketId, Label FROM TicketLabels WHERE TicketId IN (${ids}) ORDER BY TicketId, Position`, params);
    const deps = await this.sql.query<{ TicketId: string; DependsOnId: string }>(`SELECT TicketId, DependsOnId FROM TicketDependencies WHERE TicketId IN (${ids}) ORDER BY TicketId, Position`, params);
    const labelsOf = RowGroups.by(labels.rows, (l) => l.TicketId, (l) => l.Label);
    const depsOf = RowGroups.by(deps.rows, (d) => d.TicketId, (d) => d.DependsOnId);
    return rows.map((r) => ({
      id: r.Id, projectId: r.ProjectId, boardId: r.BoardId, number: r.Number, title: r.Title, description: r.Description,
      status: r.Status, priority: r.Priority,
      labels: labelsOf.get(r.Id) ?? [],
      dependsOn: depsOf.get(r.Id) ?? [],
      sortOrder: r.SortOrder, assignee: r.Assignee, needsPlanning: SqliteRunner.bool(r.NeedsPlanning), acceptance: r.Acceptance,
      plan: r.Plan, handbook: r.Handbook, diagram: r.Diagram,
      spec: r.Spec, specTask: r.SpecTask, branch: r.Branch, worktree: r.Worktree, targetBranch: r.TargetBranch,
      touches: this.jsonList(r.Touches), consumes: this.jsonList(r.Consumes), produces: this.jsonList(r.Produces),
      blockedReason: r.BlockedReason,
      plannedAt: SqliteRunner.iso(r.PlannedAt), plannedBy: r.PlannedBy, planningClaimBy: r.PlanningClaimBy,
      planningClaimAt: SqliteRunner.iso(r.PlanningClaimAt), createdAt: SqliteRunner.iso(r.CreatedAt)!,
      createdById: SqliteRunner.guid(r.CreatedById), createdByName: r.CreatedByName, updatedAt: SqliteRunner.iso(r.UpdatedAt)!, rev: r.Rev,
    }));
  }

  // Schreibt Labels und Voraussetzungen mit ihrer Position
  private async writeChildren(t: BoardTicket): Promise<void> {
    for (const [position, label] of t.labels.entries()) {
      await this.sql.query('INSERT INTO TicketLabels (TicketId, Position, Label) VALUES (@id, @position, @label)', { id: t.id, position, label });
    }
    for (const [position, dep] of t.dependsOn.entries()) {
      await this.sql.query('INSERT INTO TicketDependencies (TicketId, Position, DependsOnId) VALUES (@id, @position, @dep)', { id: t.id, position, dep });
    }
  }

  // JSON-Liste aus der DB; Unlesbares ergibt eine leere Liste
  private jsonList(json: string | null): string[] {
    try {
      const value = JSON.parse(json ?? '[]') as unknown;
      return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
    } catch {
      return [];
    }
  }

  // Parameter eines Tickets für INSERT/UPDATE; Datumswerte als Date, der Runner bindet daraus ISO-Text
  private params(t: BoardTicket): Record<string, SqliteValue> {
    return {
      id: t.id, projectId: t.projectId, boardId: t.boardId, number: t.number, title: t.title, description: t.description,
      status: t.status, priority: t.priority, sortOrder: t.sortOrder, assignee: t.assignee, needsPlanning: t.needsPlanning,
      acceptance: t.acceptance, plan: t.plan, handbook: t.handbook, diagram: t.diagram,
      spec: t.spec, specTask: t.specTask, branch: t.branch, worktree: t.worktree, targetBranch: t.targetBranch,
      touches: JSON.stringify(t.touches), consumes: JSON.stringify(t.consumes), produces: JSON.stringify(t.produces),
      blockedReason: t.blockedReason,
      plannedAt: SqliteRunner.date(t.plannedAt), plannedBy: t.plannedBy, planningClaimBy: t.planningClaimBy,
      planningClaimAt: SqliteRunner.date(t.planningClaimAt), createdAt: new Date(t.createdAt), createdById: t.createdById.toLowerCase(),
      createdByName: t.createdByName, updatedAt: new Date(t.updatedAt), rev: t.rev,
    };
  }
}
