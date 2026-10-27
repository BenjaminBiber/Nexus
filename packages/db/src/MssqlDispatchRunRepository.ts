import sql from 'mssql';
import type { DispatchConfigVersion, DispatchRun, DispatchRunFilter, DispatchRunRepository, DispatchRunSource } from '@harness/core';
import { MssqlFeatureGate } from './MssqlFeatureGate.js';
import { SqlChunks } from './SqlChunks.js';
import { SqlRunner } from './SqlRunner.js';

interface Row {
  Id: string; Kind: DispatchRun['kind']; SubjectId: string; ProjectId: number; BoardId: number | null; Agent: string; Attempt: number | null;
  Profile: string; EscalatedFrom: string | null; RequestedModel: string | null; Model: string | null; Effort: string | null;
  StartedAt: Date; EndedAt: Date | null; DurationMs: number | null; NumTurns: number | null; ExitCode: number | null; TimedOut: boolean;
  Outcome: DispatchRun['outcome']; AbortCategory: string | null; AbortReason: string | null; CostUsd: number | string | null;
  InputTokens: number | string | null; OutputTokens: number | string | null; CacheReadTokens: number | string | null;
  CacheWriteTokens: number | string | null; ModelUsage: string | null; Source: DispatchRunSource;
  MaxTurns: number | null; TimeoutMinutes: number | null; ProfileReason: DispatchRun['profileReason']; RuleIndex: number | null;
  ConfigHash: string | null; DeniedCount: number | null; DeniedTools: string | null;
}

const COLUMNS = 'Id, Kind, SubjectId, ProjectId, BoardId, Agent, Attempt, Profile, EscalatedFrom, RequestedModel, Model, Effort, StartedAt, EndedAt, ' +
  'DurationMs, NumTurns, ExitCode, TimedOut, Outcome, AbortCategory, AbortReason, CostUsd, InputTokens, OutputTokens, CacheReadTokens, ' +
  'CacheWriteTokens, ModelUsage, Source, MaxTurns, TimeoutMinutes, ProfileReason, RuleIndex, ConfigHash, DeniedCount, DeniedTools';

// Läufe des Dispatchers in board.DispatchRuns (Erweiterung dispatch-runs).
export class MssqlDispatchRunRepository implements DispatchRunRepository {
  private readonly gate: MssqlFeatureGate;

  constructor(private readonly sql: SqlRunner, missingFeatures: string[] = [], database = '') {
    this.gate = new MssqlFeatureGate(missingFeatures, database);
  }

  async insert(run: DispatchRun): Promise<boolean> {
    this.gate.require('dispatch-runs', 'dispatch-run-details');
    const values = COLUMNS.split(', ').map((c) => `@${c}`).join(', ');
    const { affected } = await this.sql.query(
      `IF NOT EXISTS (SELECT 1 FROM board.DispatchRuns WITH (UPDLOCK, HOLDLOCK) WHERE Id = @Id) INSERT INTO board.DispatchRuns (${COLUMNS}) VALUES (${values})`,
      {
        Id: run.id, Kind: run.kind, SubjectId: run.subjectId, ProjectId: run.projectId, BoardId: run.boardId, Agent: run.agent, Attempt: run.attempt,
        Profile: run.profile, EscalatedFrom: run.escalatedFrom, RequestedModel: run.requestedModel, Model: run.model, Effort: run.effort,
        StartedAt: SqlRunner.date(run.startedAt), EndedAt: SqlRunner.date(run.endedAt), DurationMs: this.int(run.durationMs), NumTurns: run.numTurns,
        ExitCode: run.exitCode, TimedOut: run.timedOut, Outcome: run.outcome, AbortCategory: run.abortCategory,
        AbortReason: run.abortReason === null ? null : run.abortReason.slice(0, 500), CostUsd: run.costUsd, InputTokens: run.inputTokens,
        OutputTokens: run.outputTokens, CacheReadTokens: run.cacheReadTokens, CacheWriteTokens: run.cacheWriteTokens, ModelUsage: run.modelUsage,
        Source: run.source, MaxTurns: run.maxTurns, TimeoutMinutes: run.timeoutMinutes, ProfileReason: run.profileReason, RuleIndex: run.ruleIndex,
        ConfigHash: run.configHash, DeniedCount: run.deniedCount, DeniedTools: run.deniedTools === null ? null : run.deniedTools.slice(0, 500),
      },
      {
        DurationMs: sql.Int, NumTurns: sql.Int, ExitCode: sql.Int, Attempt: sql.Int, BoardId: sql.Int, CostUsd: sql.Decimal(12, 6),
        InputTokens: sql.BigInt, OutputTokens: sql.BigInt, CacheReadTokens: sql.BigInt, CacheWriteTokens: sql.BigInt,
        StartedAt: sql.DateTime2(3), EndedAt: sql.DateTime2(3), TimedOut: sql.Bit, MaxTurns: sql.Int, TimeoutMinutes: sql.Int, RuleIndex: sql.Int,
        DeniedCount: sql.Int, ProfileReason: sql.VarChar(10), ConfigHash: sql.VarChar(64), DeniedTools: sql.NVarChar(500),
      },
    );
    return affected > 0;
  }

  async list(filter: DispatchRunFilter): Promise<DispatchRun[]> {
    this.gate.require('dispatch-runs', 'dispatch-run-details');
    const { rows } = await this.sql.query<Row>(
      `SELECT ${COLUMNS} FROM board.DispatchRuns WHERE ProjectId = @projectId AND StartedAt >= @from AND StartedAt < @to` +
      (filter.boardId === undefined ? '' : ' AND BoardId = @boardId') + ' ORDER BY StartedAt, Id',
      { projectId: filter.projectId, from: SqlRunner.date(filter.from), to: SqlRunner.date(filter.to), ...(filter.boardId === undefined ? {} : { boardId: filter.boardId }) },
    );
    return rows.map((r) => this.map(r));
  }

  async listAll(from: string, to: string): Promise<DispatchRun[]> {
    this.gate.require('dispatch-runs', 'dispatch-run-details');
    const { rows } = await this.sql.query<Row>(
      `SELECT ${COLUMNS} FROM board.DispatchRuns WHERE StartedAt >= @from AND StartedAt < @to ORDER BY StartedAt, Id`,
      { from: SqlRunner.date(from), to: SqlRunner.date(to) },
    );
    return rows.map((r) => this.map(r));
  }

  async firstStarted(projectId: number, source: DispatchRunSource): Promise<string | null> {
    this.gate.require('dispatch-runs', 'dispatch-run-details');
    const { rows } = await this.sql.query<{ At: Date | null }>(
      'SELECT MIN(StartedAt) AS At FROM board.DispatchRuns WHERE ProjectId = @projectId AND Source = @source', { projectId, source },
    );
    return SqlRunner.iso(rows[0]?.At ?? null);
  }

  async existing(ids: string[]): Promise<string[]> {
    this.gate.require('dispatch-runs', 'dispatch-run-details');
    const found: string[] = [];
    for (const chunk of SqlChunks.of(ids)) {
      const { list, params } = SqlChunks.params('i', chunk);
      const { rows } = await this.sql.query<{ Id: string }>(`SELECT Id FROM board.DispatchRuns WHERE Id IN (${list})`, params);
      found.push(...rows.map((r) => r.Id));
    }
    return found;
  }

  async recordConfigVersion(version: DispatchConfigVersion): Promise<boolean> {
    this.gate.require('dispatch-runs', 'dispatch-run-details');
    const { affected } = await this.sql.query(
      'IF NOT EXISTS (SELECT 1 FROM board.DispatchConfigVersions WITH (UPDLOCK, HOLDLOCK) WHERE Hash = @Hash) ' +
      'INSERT INTO board.DispatchConfigVersions (Hash, Dispatcher, FirstSeenAt, Content) VALUES (@Hash, @Dispatcher, @FirstSeenAt, @Content)',
      { Hash: version.hash, Dispatcher: version.dispatcher.slice(0, 100), FirstSeenAt: SqlRunner.date(version.firstSeenAt), Content: version.content },
      { Hash: sql.VarChar(64), Dispatcher: sql.NVarChar(100), FirstSeenAt: sql.DateTime2(3), Content: sql.NVarChar(sql.MAX) },
    );
    return affected > 0;
  }

  async listConfigVersions(from: string, to: string): Promise<DispatchConfigVersion[]> {
    this.gate.require('dispatch-runs', 'dispatch-run-details');
    const { rows } = await this.sql.query<{ Hash: string; Dispatcher: string; FirstSeenAt: Date; Content: string }>(
      'SELECT Hash, Dispatcher, FirstSeenAt, Content FROM board.DispatchConfigVersions WHERE FirstSeenAt >= @from AND FirstSeenAt < @to ' +
      'ORDER BY FirstSeenAt, Hash',
      { from: SqlRunner.date(from), to: SqlRunner.date(to) }, { from: sql.DateTime2(3), to: sql.DateTime2(3) },
    );
    return rows.map((r) => ({ hash: r.Hash, dispatcher: r.Dispatcher, firstSeenAt: SqlRunner.iso(r.FirstSeenAt)!, content: r.Content }));
  }

  // Dauer passt in int (bis ~24 Tage); größere Werte sind kaputt und werden nicht gespeichert
  private int(value: number | null): number | null {
    return value !== null && Number.isSafeInteger(value) && value >= 0 && value <= 2_147_483_647 ? value : null;
  }

  private map(r: Row): DispatchRun {
    const num = (v: number | string | null) => (v === null ? null : Number(v));
    return {
      id: r.Id, kind: r.Kind, subjectId: r.SubjectId, projectId: r.ProjectId, boardId: r.BoardId, agent: r.Agent, attempt: r.Attempt,
      profile: r.Profile, escalatedFrom: r.EscalatedFrom, requestedModel: r.RequestedModel, model: r.Model, effort: r.Effort,
      startedAt: SqlRunner.iso(r.StartedAt)!, endedAt: SqlRunner.iso(r.EndedAt), durationMs: r.DurationMs, numTurns: r.NumTurns,
      exitCode: r.ExitCode, timedOut: Boolean(r.TimedOut), outcome: r.Outcome, abortCategory: r.AbortCategory, abortReason: r.AbortReason,
      costUsd: num(r.CostUsd), inputTokens: num(r.InputTokens), outputTokens: num(r.OutputTokens), cacheReadTokens: num(r.CacheReadTokens),
      cacheWriteTokens: num(r.CacheWriteTokens), modelUsage: r.ModelUsage, source: r.Source, maxTurns: r.MaxTurns, timeoutMinutes: r.TimeoutMinutes,
      profileReason: r.ProfileReason, ruleIndex: r.RuleIndex, configHash: r.ConfigHash, deniedCount: r.DeniedCount, deniedTools: r.DeniedTools,
    };
  }
}
