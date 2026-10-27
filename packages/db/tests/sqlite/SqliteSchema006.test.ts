import { afterEach, describe, expect, it } from 'vitest';
import { SchemaVersionGuard } from '../../src/SchemaVersionGuard.js';
import { SQLITE_SCHEMA_STEPS } from '../../src/sqlite/schema/index.js';
import { SqliteSchemaInspect } from './helpers/SqliteSchemaInspect.js';

const BEFORE_006 = SQLITE_SCHEMA_STEPS.filter((s) => s.id !== '006_note_structure');

describe('SQLite-Schema-Stufe 006_note_structure', () => {
  let s: SqliteSchemaInspect | undefined;
  afterEach(() => { s?.dispose(); s = undefined; });

  it('ergänzt Notes um Titel und Zähler und trägt note-structure ein, Version bleibt 3', () => {
    s = SqliteSchemaInspect.fresh(SQLITE_SCHEMA_STEPS);
    expect(s.columns('Notes')).toEqual(expect.arrayContaining(['Title', 'SeenCount', 'SeenIn']));
    expect(s.runner.schemaFeatures()).toEqual(expect.arrayContaining(['dispatch-runs', 'note-structure']));
    expect(s.runner.schemaVersion()).toBe(3);
  });

  it('SchemaVersionGuard meldet note-structure als fehlend, solange Stufe 006 fehlt', () => {
    s = SqliteSchemaInspect.fresh(BEFORE_006);
    expect(new SchemaVersionGuard().check(s!.runner.schemaVersion(), s!.file, s!.runner.schemaFeatures()))
      .toEqual(['note-structure']);
  });
});
