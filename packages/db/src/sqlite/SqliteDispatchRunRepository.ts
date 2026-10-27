import type { DispatchConfigVersion, DispatchRun, DispatchRunFilter, DispatchRunRepository, DispatchRunSource } from '@harness/core';
import { SqlChunks } from '../SqlChunks.js';
import { SqliteRunner } from './SqliteRunner.js';

interface Row {
  Id: string; Kind: DispatchRun['kind']; SubjectId: string; ProjectId: number; BoardId: number | null; Agent: string; Attempt: number | null;
  Profile: string; EscalatedFrom: string | null; RequestedModel: string | null; Model: string | null; Effort: string | null;
  StartedAt: string; EndedAt: string | null; DurationMs: number | null; NumTurns: number | null; ExitCode: number | null; TimedOut: number;
  Outcome: DispatchRun['outcome']; AbortCategory: string | null; AbortReason: string | null; CostUsd: number | null;
  InputTokens: number | null; OutputTokens: number | null; CacheReadTokens: number | null;
  CacheWriteTokens: number | null; ModelUsage: string | null; Source: DispatchRunSource;
  MaxTurns: number | null; TimeoutMinutes: number | null; ProfileReason: DispatchRun['profileReason']; RuleIndex: number | null;
  ConfigHash: string | null; DeniedCount: number | null; DeniedTools: string | null;
}

const COLUMNS = 'Id, Kind, SubjectId, ProjectId, BoardId, Agent, Attempt, Profile, EscalatedFrom, RequestedModel, Model, Effort, StartedAt, EndedAt, ' +
  'DurationMs, NumTurns, ExitCode, TimedOut, Outcome, AbortCategory, AbortReason, CostUsd, InputTokens, OutputTokens, CacheReadTokens, ' +
  'CacheWriteTokens, ModelUsage, Source, MaxTurns, TimeoutMinutes, ProfileReason, RuleIndex, ConfigHash, DeniedCount, DeniedTools';

// Läufe des Dispatchers in DispatchRuns (Erweiterung dispatch-runs).
export class SqliteDispatchRunRepository implements DispatchRunRepository {
  constructor(private readonly sql: SqliteRunner) {}

  async insert(run: DispatchRun): Promise<boolean> {
    const values = COLUMNS.split(', ').map((c) => `@${c}`).join(', ');
    const { affected } = await this.sql.query(
      `INSERT INTO DispatchRuns (${COLUMNS}) VALUES (${values}) ON CONFLICT(Id) DO NOTHING`,
      {
        Id: run.id, Kind: run.kind, SubjectId: run.subjectId, ProjectId: run.projectId, BoardId: run.boardId, Agent: run.agent, Attempt: run.attempt,
        Profile: run.profile, EscalatedFrom: run.escalatedFrom, RequestedModel: run.requestedModel, Model: run.model, Effort: run.effort,
        StartedAt: new Date(run.startedAt), EndedAt: run.endedAt === null ? null : new Date(run.endedAt), DurationMs: this.int(run.durationMs),
        NumTurns: run.numTurns, ExitCode: run.exitCode, TimedOut: run.timedOut, Outcome: run.outcome, AbortCategory: run.abortCategory,
        AbortReason: run.abortReason === null ? null : run.abortReason.slice(0, 500), CostUsd: run.costUsd, InputTokens: run.inputTokens,
        OutputTokens: run.outputTokens, CacheReadTokens: run.cacheReadTokens, CacheWriteTokens: run.cacheWriteTokens, ModelUsage: run.modelUsage,
        Source: run.source, MaxTurns: run.maxTurns, TimeoutMinutes: run.timeoutMinutes, ProfileReason: run.profileReason, RuleIndex: run.ruleIndex,
        ConfigHash: run.configHash, DeniedCount: run.deniedCount, DeniedTools: run.deniedTools === null ? null : run.deniedTools.slice(0, 500),
      },
    );
    return affected > 0;
  }

  async list(filter: DispatchRunFilter): Promise<DispatchRun[]> {
    const { rows } = await this.sql.query<Row>(
      `SELECT ${COLUMNS} FROM DispatchRuns WHERE ProjectId = @projectId AND StartedAt >= @from AND StartedAt < @to` +
      (filter.boardId === undefined ? '' : ' AND BoardId = @boardId') + ' ORDER BY StartedAt, Id',
      { projectId: filter.projectId, from: new Date(filter.from), to: new Date(filter.to), ...(filter.boardId === undefined ? {} : { boardId: filter.boardId }) },
    );
    return rows.map((r) => this.map(r));
  }

  async listAll(from: string, to: string): Promise<DispatchRun[]> {
    const { rows } = await this.sql.query<Row>(
      `SELECT ${COLUMNS} FROM DispatchRuns WHERE StartedAt >= @from AND StartedAt < @to ORDER BY StartedAt, Id`,
      { from: new Date(from), to: new Date(to) },
    );
    return rows.map((r) => this.map(r));
  }

  async firstStarted(projectId: number, source: DispatchRunSource): Promise<string | null> {
    const { rows } = await this.sql.query<{ At: string | null }>(
      'SELECT MIN(StartedAt) AS At FROM DispatchRuns WHERE ProjectId = @projectId AND Source = @source', { projectId, source },
    );
    return SqliteRunner.iso(rows[0]?.At ?? null);
  }

  async existing(ids: string[]): Promise<string[]> {
    const found: string[] = [];
    for (const chunk of SqlChunks.of(ids)) {
      const { list, params } = SqlChunks.params('i', chunk);
      const { rows } = await this.sql.query<{ Id: string }>(`SELECT Id FROM DispatchRuns WHERE Id IN (${list})`, params);
      found.push(...rows.map((r) => r.Id));
    }
    return found;
  }

  async recordConfigVersion(version: DispatchConfigVersion): Promise<boolean> {
    const { affected } = await this.sql.query(
      'INSERT INTO DispatchConfigVersions (Hash, Dispatcher, FirstSeenAt, Content) VALUES (@Hash, @Dispatcher, @FirstSeenAt, @Content) ' +
      'ON CONFLICT(Hash) DO NOTHING',
      { Hash: version.hash, Dispatcher: version.dispatcher, FirstSeenAt: new Date(version.firstSeenAt), Content: version.content },
    );
    return affected > 0;
  }

  async listConfigVersions(from: string, to: string): Promise<DispatchConfigVersion[]> {
    const { rows } = await this.sql.query<{ Hash: string; Dispatcher: string; FirstSeenAt: string; Content: string }>(
      'SELECT Hash, Dispatcher, FirstSeenAt, Content FROM DispatchConfigVersions WHERE FirstSeenAt >= @from AND FirstSeenAt < @to ORDER BY FirstSeenAt, Hash',
      { from: new Date(from), to: new Date(to) },
    );
    return rows.map((r) => ({ hash: r.Hash, dispatcher: r.Dispatcher, firstSeenAt: SqliteRunner.iso(r.FirstSeenAt)!, content: r.Content }));
  }

  // Dauer passt in int (bis ~24 Tage); größere Werte sind kaputt und werden nicht gespeichert
  private int(value: number | null): number | null {
    return value !== null && Number.isSafeInteger(value) && value >= 0 && value <= 2_147_483_647 ? value : null;
  }

  private map(r: Row): DispatchRun {
    const num = (v: number | null) => (v === null ? null : Number(v));
    return {
      id: r.Id, kind: r.Kind, subjectId: r.SubjectId, projectId: r.ProjectId, boardId: r.BoardId, agent: r.Agent, attempt: r.Attempt,
      profile: r.Profile, escalatedFrom: r.EscalatedFrom, requestedModel: r.RequestedModel, model: r.Model, effort: r.Effort,
      startedAt: SqliteRunner.iso(r.StartedAt)!, endedAt: SqliteRunner.iso(r.EndedAt), durationMs: r.DurationMs, numTurns: r.NumTurns,
      exitCode: r.ExitCode, timedOut: SqliteRunner.bool(r.TimedOut), outcome: r.Outcome, abortCategory: r.AbortCategory, abortReason: r.AbortReason,
      costUsd: num(r.CostUsd), inputTokens: num(r.InputTokens), outputTokens: num(r.OutputTokens), cacheReadTokens: num(r.CacheReadTokens),
      cacheWriteTokens: num(r.CacheWriteTokens), modelUsage: r.ModelUsage, source: r.Source, maxTurns: r.MaxTurns, timeoutMinutes: r.TimeoutMinutes,
      profileReason: r.ProfileReason, ruleIndex: r.RuleIndex, configHash: r.ConfigHash, deniedCount: r.DeniedCount, deniedTools: r.DeniedTools,
    };
  }
}
