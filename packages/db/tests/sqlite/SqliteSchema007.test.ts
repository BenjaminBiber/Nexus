import { afterEach, describe, expect, it } from 'vitest';
import { SchemaVersionGuard } from '../../src/SchemaVersionGuard.js';
import { SQLITE_SCHEMA_STEPS } from '../../src/sqlite/schema/index.js';
import { SqliteSchemaInspect } from './helpers/SqliteSchemaInspect.js';

const BEFORE_007 = SQLITE_SCHEMA_STEPS.filter((s) => s.id !== '007_dispatch_run_details');
const NEW_COLUMNS = ['MaxTurns', 'TimeoutMinutes', 'ProfileReason', 'RuleIndex', 'ConfigHash', 'DeniedCount', 'DeniedTools'];

// Lauf-Zeile mit Pflichtfeldern; die neuen Spalten bleiben offen, soweit sie nicht übergeben werden
const insertRun = (s: SqliteSchemaInspect, id: string, extraColumns = '', extraValues = '', params: Record<string, string> = {}) =>
  s.run(
    `INSERT INTO DispatchRuns (Id, Kind, SubjectId, ProjectId, Agent, Profile, StartedAt, Outcome, Source${extraColumns})
     VALUES (@id, 'work', 'AIH-0001', 1, 'junior', 'standard', @now, 'done', 'dispatcher'${extraValues})`,
    { id, now: SqliteSchemaInspect.NOW, ...params },
  );

describe('SQLite-Schema-Stufe 007_dispatch_run_details', () => {
  let s: SqliteSchemaInspect | undefined;
  afterEach(() => { s?.dispose(); s = undefined; });

  it('ergänzt die Lauf-Spalten, legt die Konfig-Versionen an und trägt dispatch-run-details ein, Version bleibt 3', () => {
    s = SqliteSchemaInspect.fresh(SQLITE_SCHEMA_STEPS);
    expect(s.columns('DispatchRuns')).toEqual(expect.arrayContaining(NEW_COLUMNS));
    expect(s.tables()).toContain('DispatchConfigVersions');
    expect(s.columns('DispatchConfigVersions')).toEqual(['Hash', 'Dispatcher', 'FirstSeenAt', 'Content']);
    expect(s.primaryKey('DispatchConfigVersions')).toEqual(['Hash']);
    expect(Object.keys(s.indexes('DispatchRuns'))).toContain('IX_DispatchRuns_Config');
    expect(Object.keys(s.indexes('DispatchConfigVersions'))).toContain('IX_DispatchConfigVersions_FirstSeen');
    expect(s.runner.schemaFeatures()).toEqual(expect.arrayContaining(['note-structure', 'dispatch-run-details']));
    expect(s.runner.schemaVersion()).toBe(3);
  });

  it('nimmt erlaubte Gründe der Profilwahl an und lehnt andere ab', () => {
    s = SqliteSchemaInspect.fresh(SQLITE_SCHEMA_STEPS);
    s.seedBoard();
    insertRun(s, 'r1', ', ProfileReason, RuleIndex', ', @reason, 2', { reason: 'rule' });
    expect(s.all<{ ProfileReason: string }>('SELECT ProfileReason FROM DispatchRuns WHERE Id = @i', { i: 'r1' })[0]?.ProfileReason).toBe('rule');
    insertRun(s, 'r2');
    expect(s.all<{ ProfileReason: string | null }>('SELECT ProfileReason FROM DispatchRuns WHERE Id = @i', { i: 'r2' })[0]?.ProfileReason).toBeNull();
    expect(() => insertRun(s!, 'r3', ', ProfileReason', ', @reason', { reason: 'irgendwas' })).toThrow(/CK_DispatchRuns_ProfileReason|CHECK/i);
  });

  it('SchemaVersionGuard meldet dispatch-run-details, solange Stufe 007 fehlt', () => {
    s = SqliteSchemaInspect.fresh(BEFORE_007);
    // Fehlende Erweiterungen werfen nicht mehr, sie werden gemeldet; nur der Zugriff auf die neuen Spalten verweigert sich
    expect(new SchemaVersionGuard().check(s!.runner.schemaVersion(), s!.file, s!.runner.schemaFeatures()))
      .toEqual(['dispatch-run-details']);
    expect(SchemaVersionGuard.featureMessage('dispatch-run-details', 'AiHarness'))
      .toContain('006_dispatch_run_details.sql');
  });

  it('ändert beim zweiten Öffnen nichts', () => {
    s = SqliteSchemaInspect.fresh(SQLITE_SCHEMA_STEPS);
    const before = s.columns('DispatchRuns');
    s.runner.run();
    expect(s.columns('DispatchRuns')).toEqual(before);
    expect(s.runner.schemaFeatures().filter((f) => f === 'dispatch-run-details')).toHaveLength(1);
  });
});
