import { HarnessError, type BoardTicket, type BoardTicketFilter, type BoardTicketRepository, type BoardTicketStatus, type Priority } from '@harness/core';
import { MssqlFeatureGate } from './MssqlFeatureGate.js';
import { RowGroups } from './RowGroups.js';
import { SqlChunks } from './SqlChunks.js';
import { SqlRunner, type SqlValue } from './SqlRunner.js';

interface Row {
  Id: string; ProjectId: number; BoardId: number; Number: number; Title: string; Description: string;
  Status: BoardTicketStatus; Priority: Priority; SortOrder: number; Assignee: string | null; NeedsPlanning: boolean;
  Acceptance: string; Plan: string; Handbook: string; Diagram: string; BlockedReason: string | null;
  PlannedAt: Date | null; PlannedBy: string | null; PlanningClaimBy: string | null; PlanningClaimAt: Date | null;
  CreatedAt: Date; CreatedById: string; CreatedByName: string; UpdatedAt: Date; Rev: number;
  Spec: string | null; SpecTask: string | null; Branch: string | null; Worktree: string | null; TargetBranch: string | null; Touches: string; Consumes: string; Produces: string;
}

const COLUMNS = `t.Id, t.ProjectId, t.BoardId, t.Number, t.Title, t.Description, t.Status, t.Priority, t.SortOrder, t.Assignee,
  t.NeedsPlanning, t.Acceptance, t.[Plan], t.Handbook, t.Diagram, t.BlockedReason, t.PlannedAt, t.PlannedBy,
  t.PlanningClaimBy, t.PlanningClaimAt, t.CreatedAt, t.CreatedById, t.CreatedByName, t.UpdatedAt, t.Rev,
  t.Spec, t.SpecTask, t.Branch, t.Worktree, t.Touches, t.Consumes, t.Produces`;

// Wie COLUMNS, aber die langen Texte nur als Kennzeichen (filter.withoutTexts): leer bleibt leer, sonst 'x'
const LIGHT_COLUMNS = ['Description', 'Acceptance', '[Plan]', 'Handbook', 'Diagram']
  .reduce((columns, name) => columns.replace(`t.${name}`, `CASE WHEN LEN(LTRIM(t.${name})) > 0 THEN N'x' ELSE N'' END AS ${name}`), COLUMNS);

// Tickets in board.Tickets samt Labels (board.TicketLabels) und Voraussetzungen (board.TicketDependencies).
// Ohne Erweiterung target-branch (007) fehlt die Spalte TargetBranch: gelesen wird dann null, geschrieben nur ohne Zielbranch.
export class MssqlTicketRepository implements BoardTicketRepository {
  private readonly gate: MssqlFeatureGate;

  constructor(private readonly sql: SqlRunner, missingFeatures: string[] = [], database = '') {
    this.gate = new MssqlFeatureGate(missingFeatures, database);
  }

  // Ob die Spalte TargetBranch da ist; ein gesetzter Zielbranch verlangt sie
  private hasTarget(t?: BoardTicket): boolean {
    if (this.gate.has('target-branch')) return true;
    if (t?.targetBranch) this.gate.require('target-branch');
    return false;
  }

  // forUpdate sperrt die Zeile bis zum Ende der Transaktion
  async byId(id: string, forUpdate = false): Promise<BoardTicket | null> {
    const hint = forUpdate ? 'WITH (UPDLOCK, ROWLOCK)' : '';
    const found = await this.load(`FROM board.Tickets t ${hint} WHERE t.Id = @id`, { id });
    return found[0] ?? null;
  }

  // Sortiert nach Projekt und Nummer, damit die Reihenfolge stabil ist
  async list(filter: BoardTicketFilter = {}): Promise<BoardTicket[]> {
    const where: string[] = [];
    const params: Record<string, SqlValue> = {};
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
    return this.load(`FROM board.Tickets t ${where.length ? `WHERE ${where.join(' AND ')}` : ''}`, params, filter.withoutTexts);
  }

  async openCountByBoard(boardIds: number[]): Promise<Record<number, number>> {
    if (!boardIds.length) return {};
    const { list, params } = SqlChunks.params('b', boardIds);
    const { rows } = await this.sql.query<{ BoardId: number; N: number }>(
      `SELECT BoardId, COUNT(*) AS N FROM board.Tickets
       WHERE BoardId IN (${list}) AND Status NOT IN ('done', 'not_needed') GROUP BY BoardId`,
      params,
    );
    return Object.fromEntries(rows.map((r) => [r.BoardId, r.N]));
  }

  async insert(t: BoardTicket): Promise<void> {
    const target = this.hasTarget(t);
    await this.sql.query(
      `INSERT INTO board.Tickets (Id, ProjectId, BoardId, Number, Title, Description, Status, Priority, SortOrder, Assignee,
         NeedsPlanning, Acceptance, [Plan], Handbook, Diagram, BlockedReason, PlannedAt, PlannedBy, PlanningClaimBy,
         PlanningClaimAt, CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev,
         Spec, SpecTask, Branch, Worktree, ${target ? 'TargetBranch, ' : ''}Touches, Consumes, Produces)
       VALUES (@id, @projectId, @boardId, @number, @title, @description, @status, @priority, @sortOrder, @assignee,
         @needsPlanning, @acceptance, @plan, @handbook, @diagram, @blockedReason, @plannedAt, @plannedBy, @planningClaimBy,
         @planningClaimAt, @createdAt, @createdById, @createdByName, @updatedAt, @rev,
         @spec, @specTask, @branch, @worktree, ${target ? '@targetBranch, ' : ''}@touches, @consumes, @produces)`,
      this.params(t),
    );
    await this.writeChildren(t);
  }

  // Schreibt nur, wenn in der DB noch die Vorgängerversion steht
  async update(t: BoardTicket): Promise<void> {
    const target = this.hasTarget(t);
    const { affected } = await this.sql.query(
      `UPDATE board.Tickets SET BoardId = @boardId, Title = @title, Description = @description, Status = @status,
         Priority = @priority, SortOrder = @sortOrder, Assignee = @assignee, NeedsPlanning = @needsPlanning,
         Acceptance = @acceptance, [Plan] = @plan, Handbook = @handbook, Diagram = @diagram, BlockedReason = @blockedReason,
         PlannedAt = @plannedAt, PlannedBy = @plannedBy, PlanningClaimBy = @planningClaimBy, PlanningClaimAt = @planningClaimAt,
         Spec = @spec, SpecTask = @specTask, Branch = @branch, Worktree = @worktree, ${target ? 'TargetBranch = @targetBranch, ' : ''}Touches = @touches, Consumes = @consumes, Produces = @produces,
         UpdatedAt = @updatedAt, Rev = @rev
       WHERE Id = @id AND Rev = @previousRev`,
      { ...this.params(t), previousRev: t.rev - 1 },
    );
    if (affected === 0) throw new HarnessError('conflict', `Ticket ${t.id} wurde inzwischen geändert. Bitte neu lesen.`);
    await this.sql.query('DELETE FROM board.TicketLabels WHERE TicketId = @id; DELETE FROM board.TicketDependencies WHERE TicketId = @id;', { id: t.id });
    await this.writeChildren(t);
  }

  // Labels, eigene Voraussetzungen, Kommentare und Verlauf fallen per Kaskade mit
  async delete(id: string): Promise<void> {
    await this.sql.query('DELETE FROM board.TicketDependencies WHERE DependsOnId = @id; DELETE FROM board.Tickets WHERE Id = @id;', { id });
  }

  // Sortiert in JS, damit die Reihenfolge nicht von der DB-Collation abhängt
  async dependents(id: string): Promise<string[]> {
    const { rows } = await this.sql.query<{ TicketId: string }>('SELECT TicketId FROM board.TicketDependencies WHERE DependsOnId = @id', { id });
    return rows.map((r) => r.TicketId).sort();
  }

  async removeDependencyOn(id: string): Promise<string[]> {
    const { rows } = await this.sql.query<{ TicketId: string }>('DELETE FROM board.TicketDependencies OUTPUT deleted.TicketId WHERE DependsOnId = @id', { id });
    return rows.map((r) => r.TicketId).sort();
  }

  async maxSortOrder(boardId: number, status: BoardTicketStatus): Promise<number | null> {
    const { rows } = await this.sql.query<{ M: number | null }>('SELECT MAX(SortOrder) AS M FROM board.Tickets WHERE BoardId = @boardId AND Status = @status', { boardId, status });
    return rows[0]?.M ?? null;
  }

  // Lädt Tickets zu einer FROM/WHERE-Klausel und ergänzt Labels und Voraussetzungen
  private async load(from: string, params: Record<string, SqlValue>, withoutTexts = false): Promise<BoardTicket[]> {
    const columns = withoutTexts ? LIGHT_COLUMNS : COLUMNS;
    const { rows } = await this.sql.query<Row>(`SELECT ${columns}, ${this.hasTarget() ? 't.TargetBranch' : 'CAST(NULL AS nvarchar(400)) AS TargetBranch'} ${from} ORDER BY t.ProjectId, t.Number`, params);
    if (!rows.length) return [];
    const ids = `SELECT t.Id ${from.replace(/WITH \(UPDLOCK, ROWLOCK\)/, '')}`;
    const labels = await this.sql.query<{ TicketId: string; Label: string }>(`SELECT TicketId, Label FROM board.TicketLabels WHERE TicketId IN (${ids}) ORDER BY TicketId, Position`, params);
    const deps = await this.sql.query<{ TicketId: string; DependsOnId: string }>(`SELECT TicketId, DependsOnId FROM board.TicketDependencies WHERE TicketId IN (${ids}) ORDER BY TicketId, Position`, params);
    const labelsOf = RowGroups.by(labels.rows, (l) => l.TicketId, (l) => l.Label);
    const depsOf = RowGroups.by(deps.rows, (d) => d.TicketId, (d) => d.DependsOnId);
    return rows.map((r) => ({
      id: r.Id, projectId: r.ProjectId, boardId: r.BoardId, number: r.Number, title: r.Title, description: r.Description,
      status: r.Status, priority: r.Priority,
      labels: labelsOf.get(r.Id) ?? [],
      dependsOn: depsOf.get(r.Id) ?? [],
      sortOrder: r.SortOrder, assignee: r.Assignee, needsPlanning: Boolean(r.NeedsPlanning), acceptance: r.Acceptance,
      plan: r.Plan, handbook: r.Handbook, diagram: r.Diagram,
      spec: r.Spec, specTask: r.SpecTask, branch: r.Branch, worktree: r.Worktree, targetBranch: r.TargetBranch,
      touches: this.jsonList(r.Touches), consumes: this.jsonList(r.Consumes), produces: this.jsonList(r.Produces),
      blockedReason: r.BlockedReason,
      plannedAt: SqlRunner.iso(r.PlannedAt), plannedBy: r.PlannedBy, planningClaimBy: r.PlanningClaimBy,
      planningClaimAt: SqlRunner.iso(r.PlanningClaimAt), createdAt: SqlRunner.iso(r.CreatedAt)!,
      createdById: SqlRunner.guid(r.CreatedById), createdByName: r.CreatedByName, updatedAt: SqlRunner.iso(r.UpdatedAt)!, rev: r.Rev,
    }));
  }

  // Schreibt Labels und Voraussetzungen mit ihrer Position
  private async writeChildren(t: BoardTicket): Promise<void> {
    for (const [position, label] of t.labels.entries()) {
      await this.sql.query('INSERT INTO board.TicketLabels (TicketId, Position, Label) VALUES (@id, @position, @label)', { id: t.id, position, label });
    }
    for (const [position, dep] of t.dependsOn.entries()) {
      await this.sql.query('INSERT INTO board.TicketDependencies (TicketId, Position, DependsOnId) VALUES (@id, @position, @dep)', { id: t.id, position, dep });
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

  // Parameter eines Tickets für INSERT/UPDATE
  private params(t: BoardTicket): Record<string, SqlValue> {
    return {
      id: t.id, projectId: t.projectId, boardId: t.boardId, number: t.number, title: t.title, description: t.description,
      status: t.status, priority: t.priority, sortOrder: t.sortOrder, assignee: t.assignee, needsPlanning: t.needsPlanning,
      acceptance: t.acceptance, plan: t.plan, handbook: t.handbook, diagram: t.diagram,
      spec: t.spec, specTask: t.specTask, branch: t.branch, worktree: t.worktree, targetBranch: t.targetBranch,
      touches: JSON.stringify(t.touches), consumes: JSON.stringify(t.consumes), produces: JSON.stringify(t.produces),
      blockedReason: t.blockedReason,
      plannedAt: SqlRunner.date(t.plannedAt), plannedBy: t.plannedBy, planningClaimBy: t.planningClaimBy,
      planningClaimAt: SqlRunner.date(t.planningClaimAt), createdAt: SqlRunner.date(t.createdAt), createdById: t.createdById,
      createdByName: t.createdByName, updatedAt: SqlRunner.date(t.updatedAt), rev: t.rev,
    };
  }
}
