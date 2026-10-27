import { afterEach, describe, expect, it } from 'vitest';
import { SchemaVersionGuard } from '../../src/SchemaVersionGuard.js';
import { step001Initial } from '../../src/sqlite/schema/001_initial.js';
import { step002Questions } from '../../src/sqlite/schema/002_questions.js';
import { step003Harness } from '../../src/sqlite/schema/003_harness.js';
import { step004DispatchRuns } from '../../src/sqlite/schema/004_dispatch_runs.js';
import { SQLITE_SCHEMA_STEPS } from '../../src/sqlite/schema/index.js';
import { SqliteSchemaInspect } from './helpers/SqliteSchemaInspect.js';

const UP_TO_004 = [step001Initial, step002Questions, step003Harness, step004DispatchRuns];

describe('SQLite-Schema-Stufe 004_dispatch_runs', () => {
  let s: SqliteSchemaInspect | undefined;
  afterEach(() => { s?.dispose(); s = undefined; });

  it('legt DispatchRuns und SchemaFeatures an und trägt dispatch-runs ein, Version bleibt 3', () => {
    s = SqliteSchemaInspect.fresh(UP_TO_004);
    expect(s.columns('SchemaFeatures')).toEqual(['Name', 'Script', 'AppliedAt']);
    expect(s.columns('DispatchRuns')).toEqual([
      'Id', 'Kind', 'SubjectId', 'ProjectId', 'BoardId', 'Agent', 'Attempt', 'Profile', 'EscalatedFrom', 'RequestedModel', 'Model', 'Effort',
      'StartedAt', 'EndedAt', 'DurationMs', 'NumTurns', 'ExitCode', 'TimedOut', 'Outcome', 'AbortCategory', 'AbortReason', 'CostUsd',
      'InputTokens', 'OutputTokens', 'CacheReadTokens', 'CacheWriteTokens', 'ModelUsage', 'Source', 'RecordedAt',
    ]);
    expect(s.column('DispatchRuns', 'CostUsd')?.type).toBe('REAL');
    expect(s.indexes('DispatchRuns')).toEqual({ IX_DispatchRuns_Project_Started: ['ProjectId', 'StartedAt'], IX_DispatchRuns_Subject: ['SubjectId'] });
    expect(s.indexes('History')).toEqual({ IX_History_Ticket: ['TicketId'], IX_History_Field_At: ['Field', 'At'] });
    expect(s.foreignKeys('DispatchRuns')).toEqual([]);
    expect(s.runner.schemaFeatures()).toEqual(['dispatch-runs']);
    expect(s.all('SELECT Name, Script FROM SchemaFeatures')).toEqual([{ Name: 'dispatch-runs', Script: '004_dispatch_runs' }]);
    expect(s.runner.schemaVersion()).toBe(3);
  });

  it('speichert Kosten, Tokens und Defaults und prüft Kind und Source', () => {
    s = SqliteSchemaInspect.fresh(UP_TO_004);
    const insert = (id: string, kind: string, source: string) => s!.run(
      `INSERT INTO DispatchRuns (Id, Kind, SubjectId, ProjectId, Agent, Profile, StartedAt, Outcome, Source, CostUsd, InputTokens)
       VALUES (@id, @kind, 'AIH-0001', 1, 'claude', 'standard', @now, 'done', @source, 0.123456, 1234567890123)`,
      { id, kind, source, now: SqliteSchemaInspect.NOW },
    );
    insert('run-1', 'work', 'dispatcher');
    expect(s.all('SELECT CostUsd, InputTokens, TimedOut FROM DispatchRuns')[0]).toEqual({ CostUsd: 0.123456, InputTokens: 1234567890123, TimedOut: 0 });
    expect(s.all('SELECT RecordedAt FROM DispatchRuns')[0].RecordedAt).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/);
    expect(() => insert('run-2', 'spielen', 'dispatcher')).toThrow(/CK_DispatchRuns_Kind/);
    expect(() => insert('run-3', 'merge', 'hand')).toThrow(/CK_DispatchRuns_Source/);
  });

  it('SchemaVersionGuard akzeptiert eine frische SQLite-Datei mit allen Stufen', () => {
    s = SqliteSchemaInspect.fresh(SQLITE_SCHEMA_STEPS);
    expect(new SchemaVersionGuard().check(s!.runner.schemaVersion(), s!.file, s!.runner.schemaFeatures())).toEqual([]);
  });

  it('SchemaVersionGuard meldet dispatch-runs als fehlend, solange Stufe 004 fehlt', () => {
    s = SqliteSchemaInspect.fresh([step001Initial, step002Questions, step003Harness]);
    expect(new SchemaVersionGuard().check(s!.runner.schemaVersion(), s!.file, s!.runner.schemaFeatures()))
      .toContain('dispatch-runs');
  });
});
