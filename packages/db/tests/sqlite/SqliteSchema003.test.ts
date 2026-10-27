import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { EXPECTED_SCHEMA_VERSION } from '../../src/SchemaVersionGuard.js';
import { step001Initial } from '../../src/sqlite/schema/001_initial.js';
import { step002Questions } from '../../src/sqlite/schema/002_questions.js';
import { step003Harness } from '../../src/sqlite/schema/003_harness.js';
import { SqliteSchemaInspect } from './helpers/SqliteSchemaInspect.js';

describe('SQLite-Schema-Stufe 003_harness', () => {
  let s: SqliteSchemaInspect;
  beforeEach(() => {
    s = SqliteSchemaInspect.fresh([step001Initial, step002Questions, step003Harness]);
    s.seedBoard();
  });
  afterEach(() => s.dispose());

  it('meldet Schema-Version 3 (wie EXPECTED_SCHEMA_VERSION)', () => {
    expect(s.runner.schemaVersion()).toBe(3);
    expect(s.runner.schemaVersion()).toBe(EXPECTED_SCHEMA_VERSION);
    expect(s.all('SELECT Version FROM SchemaVersion ORDER BY Version')).toEqual([{ Version: 1 }, { Version: 2 }, { Version: 3 }]);
  });

  it('hängt die Harness-Spalten an Tickets und die Lesemarke an Comments an', () => {
    expect(s.columns('Tickets').slice(-7)).toEqual(['Spec', 'SpecTask', 'Branch', 'Worktree', 'Touches', 'Consumes', 'Produces']);
    expect(s.columns('Tickets')).toHaveLength(33);
    for (const name of ['Touches', 'Consumes', 'Produces']) {
      expect(s.column('Tickets', name), name).toEqual({ type: 'TEXT', notnull: 1, dflt_value: "'[]'" });
    }
    expect(s.column('Tickets', 'Spec')).toEqual({ type: 'TEXT', notnull: 0, dflt_value: null });
    expect(s.columns('Comments').at(-1)).toBe('ReadByAgentAt');
    s.insertTicket('AIH-0001', 1);
    expect(s.all('SELECT Spec, SpecTask, Branch, Worktree, Touches, Consumes, Produces FROM Tickets')[0])
      .toEqual({ Spec: null, SpecTask: null, Branch: null, Worktree: null, Touches: '[]', Consumes: '[]', Produces: '[]' });
  });

  it('legt Schritte und Sessions je Ticket mit Kaskade an', () => {
    expect(s.columns('TicketSteps')).toEqual(['TicketId', 'Position', 'Text', 'Done', 'Detail']);
    expect(s.primaryKey('TicketSteps')).toEqual(['TicketId', 'Position']);
    expect(s.columns('TicketSessions')).toEqual(['TicketId', 'SessionId', 'AddedAt']);
    expect(s.primaryKey('TicketSessions')).toEqual(['TicketId', 'SessionId']);
    expect(s.indexes('TicketSessions')).toEqual({ IX_TicketSessions_Session: ['SessionId'] });
    expect(s.foreignKeys('TicketSteps')).toEqual([{ from: 'TicketId', table: 'Tickets', to: 'Id', onDelete: 'CASCADE' }]);
    expect(s.foreignKeys('TicketSessions')).toEqual([{ from: 'TicketId', table: 'Tickets', to: 'Id', onDelete: 'CASCADE' }]);
    s.insertTicket('AIH-0001', 1);
    s.run("INSERT INTO TicketSteps (TicketId, Position, Text) VALUES ('AIH-0001', 0, 'Schritt')");
    s.run("INSERT INTO TicketSessions (TicketId, SessionId, AddedAt) VALUES ('AIH-0001', 'sess-1', @now)", { now: SqliteSchemaInspect.NOW });
    expect(s.all('SELECT Done, Detail FROM TicketSteps')[0]).toEqual({ Done: 0, Detail: null });
    s.run("DELETE FROM Tickets WHERE Id = 'AIH-0001'");
    expect(s.all('SELECT COUNT(*) AS N FROM TicketSteps')[0].N).toBe(0);
    expect(s.all('SELECT COUNT(*) AS N FROM TicketSessions')[0].N).toBe(0);
    expect(s.foreignKeyCheck()).toEqual([]);
  });
});
