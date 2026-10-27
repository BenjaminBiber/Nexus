import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { step001Initial } from '../../src/sqlite/schema/001_initial.js';
import { SqliteSchemaInspect } from './helpers/SqliteSchemaInspect.js';

const ISO = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;

describe('SQLite-Schema-Stufe 001_initial', () => {
  let s: SqliteSchemaInspect;
  beforeEach(() => { s = SqliteSchemaInspect.fresh([step001Initial]); });
  afterEach(() => s.dispose());

  it('legt alle Tabellen aus 001 an und meldet Schema-Version 1', () => {
    expect(s.tables()).toEqual(['Boards', 'Comments', 'Deletions', 'History', 'Notes', 'Projects', 'SchemaVersion', 'TicketDependencies', 'TicketLabels', 'Tickets', 'Users']);
    expect(s.runner.schemaVersion()).toBe(1);
  });

  it('übernimmt die Spaltennamen von SQL Server in gleicher Reihenfolge', () => {
    expect(s.columns('SchemaVersion')).toEqual(['Version', 'AppliedAt']);
    expect(s.columns('Users')).toEqual(['Id', 'Name', 'CreatedAt']);
    expect(s.columns('Projects')).toEqual(['Id', 'Key', 'Name', 'NextTicketNumber', 'CreatedAt', 'CreatedBy']);
    expect(s.columns('Boards')).toEqual(['Id', 'ProjectId', 'OwnerId', 'Name', 'RepoPath', 'CreatedAt', 'RowVer']);
    expect(s.columns('Tickets')).toEqual([
      'Id', 'ProjectId', 'BoardId', 'Number', 'Title', 'Description', 'Status', 'Priority', 'SortOrder', 'Assignee', 'NeedsPlanning',
      'Acceptance', 'Plan', 'Handbook', 'Diagram', 'BlockedReason', 'PlannedAt', 'PlannedBy', 'PlanningClaimBy', 'PlanningClaimAt',
      'CreatedAt', 'CreatedById', 'CreatedByName', 'UpdatedAt', 'Rev', 'RowVer',
    ]);
    expect(s.columns('TicketLabels')).toEqual(['TicketId', 'Position', 'Label']);
    expect(s.columns('TicketDependencies')).toEqual(['TicketId', 'Position', 'DependsOnId']);
    expect(s.columns('Comments')).toEqual(['Id', 'TicketId', 'AuthorId', 'AuthorName', 'Body', 'CreatedAt', 'RowVer']);
    expect(s.columns('History')).toEqual(['Id', 'TicketId', 'At', 'ById', 'ByName', 'Field', 'FromValue', 'ToValue', 'Note']);
    expect(s.columns('Notes')).toEqual(['Id', 'BoardId', 'AuthorId', 'AuthorName', 'Body', 'Source', 'Labels', 'State', 'TicketId', 'Reason', 'CreatedAt', 'RowVer']);
    expect(s.columns('Deletions')).toEqual(['Id', 'At', 'ById', 'ByName', 'TicketId', 'TicketTitle', 'FromStatus']);
    expect(s.column('Tickets', 'RowVer')).toEqual({ type: 'INTEGER', notnull: 1, dflt_value: '0' });
  });

  it('legt Indizes, Primär- und Eindeutigkeitsschlüssel an', () => {
    expect(s.indexes('Boards')).toEqual({ IX_Boards_Project: ['ProjectId'] });
    expect(s.indexes('Tickets')).toEqual({ IX_Tickets_Board: ['BoardId', 'Status'] });
    expect(s.indexes('TicketDependencies')).toEqual({ IX_TicketDeps_DependsOn: ['DependsOnId'] });
    expect(s.indexes('Comments')).toEqual({ IX_Comments_Ticket: ['TicketId'] });
    expect(s.indexes('History')).toEqual({ IX_History_Ticket: ['TicketId'] });
    expect(s.indexes('Notes')).toEqual({ IX_Notes_Board: ['BoardId', 'State'] });
    expect(s.uniques('Projects')).toEqual([['Key']]);
    expect(s.uniques('Tickets')).toEqual([['ProjectId', 'Number']]);
    expect(s.primaryKey('Tickets')).toEqual(['Id']);
    expect(s.primaryKey('TicketLabels')).toEqual(['TicketId', 'Label']);
    expect(s.primaryKey('TicketDependencies')).toEqual(['TicketId', 'DependsOnId']);
  });

  it('übernimmt Fremdschlüssel und Kaskaden wie im Original', () => {
    expect(s.foreignKeys('Tickets')).toEqual([
      { from: 'BoardId', table: 'Boards', to: 'Id', onDelete: 'NO ACTION' },
      { from: 'CreatedById', table: 'Users', to: 'Id', onDelete: 'NO ACTION' },
      { from: 'ProjectId', table: 'Projects', to: 'Id', onDelete: 'NO ACTION' },
    ]);
    expect(s.foreignKeys('TicketDependencies')).toEqual([
      { from: 'DependsOnId', table: 'Tickets', to: 'Id', onDelete: 'NO ACTION' },
      { from: 'TicketId', table: 'Tickets', to: 'Id', onDelete: 'CASCADE' },
    ]);
    expect(s.foreignKeys('TicketLabels')).toEqual([{ from: 'TicketId', table: 'Tickets', to: 'Id', onDelete: 'CASCADE' }]);
    expect(s.foreignKeys('Comments').map((f) => `${f.from}:${f.onDelete}`)).toEqual(['AuthorId:NO ACTION', 'TicketId:CASCADE']);
    expect(s.foreignKeys('History').map((f) => `${f.from}:${f.onDelete}`)).toEqual(['ById:NO ACTION', 'TicketId:CASCADE']);
    expect(s.foreignKeys('Notes').map((f) => `${f.from}:${f.onDelete}`)).toEqual(['AuthorId:NO ACTION', 'BoardId:CASCADE']);
    expect(s.foreignKeys('Deletions')).toEqual([{ from: 'ById', table: 'Users', to: 'Id', onDelete: 'NO ACTION' }]);
    expect(s.foreignKeyCheck()).toEqual([]);
  });

  it('setzt Defaults wie SQL Server', () => {
    s.seedBoard();
    s.insertTicket('AIH-0001', 1);
    expect(s.all('SELECT CreatedAt FROM Users')[0].CreatedAt).toMatch(ISO);
    expect(s.all('SELECT AppliedAt FROM SchemaVersion')[0].AppliedAt).toMatch(ISO);
    expect(s.all('SELECT NextTicketNumber FROM Projects')[0].NextTicketNumber).toBe(1);
    expect(s.all('SELECT Description, Acceptance, "Plan", Handbook, Diagram, RowVer FROM Tickets')[0])
      .toEqual({ Description: '', Acceptance: '', Plan: '', Handbook: '', Diagram: '', RowVer: 0 });
  });

  it('lehnt CHECK-, Fremdschlüssel- und Eindeutigkeitsverstöße ab', () => {
    s.seedBoard();
    expect(() => s.insertTicket('AIH-0001', 1, 'foo')).toThrow(/CHECK constraint failed: CK_Tickets_Status/);
    expect(() => s.insertTicket('AIH-0001', 1, 'backlog', 'mega')).toThrow(/CHECK constraint failed: CK_Tickets_Priority/);
    expect(() => s.run("INSERT INTO Notes (BoardId, AuthorId, AuthorName, Body, State, CreatedAt) VALUES (1, @u, 'T', 'b', 'weg', @now)",
      { u: SqliteSchemaInspect.USER, now: SqliteSchemaInspect.NOW })).toThrow(/CHECK constraint failed: CK_Notes_State/);
    expect(() => s.insertTicket('AIH-0001', 1, 'backlog', 'normal', 99)).toThrow(/FOREIGN KEY constraint failed/);
    s.insertTicket('AIH-0001', 1);
    expect(() => s.insertTicket('AIH-0002', 1)).toThrow(/UNIQUE constraint failed: Tickets\.ProjectId, Tickets\.Number/);
  });

  it('löscht Labels, Kommentare und Verlauf mit dem Ticket; Voraussetzungs-Verweise blockieren', () => {
    s.seedBoard();
    s.insertTicket('AIH-0001', 1);
    s.insertTicket('AIH-0002', 2);
    const p = { u: SqliteSchemaInspect.USER, now: SqliteSchemaInspect.NOW };
    s.run("INSERT INTO TicketLabels (TicketId, Position, Label) VALUES ('AIH-0001', 0, 'db')");
    s.run("INSERT INTO TicketDependencies (TicketId, Position, DependsOnId) VALUES ('AIH-0002', 0, 'AIH-0001')");
    s.run("INSERT INTO Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt) VALUES ('AIH-0001', @u, 'T', 'Text', @now)", p);
    s.run("INSERT INTO History (TicketId, At, ById, ByName, Field) VALUES ('AIH-0001', @now, @u, 'T', 'status')", p);
    expect(() => s.run("DELETE FROM Tickets WHERE Id = 'AIH-0001'")).toThrow(/FOREIGN KEY constraint failed/);
    s.run("DELETE FROM TicketDependencies WHERE DependsOnId = 'AIH-0001'");
    s.run("DELETE FROM Tickets WHERE Id = 'AIH-0001'");
    for (const table of ['TicketLabels', 'Comments', 'History']) {
      expect(s.all(`SELECT COUNT(*) AS N FROM ${table}`)[0].N, table).toBe(0);
    }
  });

  it('vergibt Board-Ids nach dem Löschen nicht neu (AUTOINCREMENT)', () => {
    s.seedBoard();
    s.run('DELETE FROM Boards WHERE Id = 1');
    s.run("INSERT INTO Boards (ProjectId, OwnerId, Name, CreatedAt) VALUES (1, @u, 'Neu', @now)", { u: SqliteSchemaInspect.USER, now: SqliteSchemaInspect.NOW });
    expect(s.all('SELECT Id FROM Boards')).toEqual([{ Id: 2 }]);
  });
});
