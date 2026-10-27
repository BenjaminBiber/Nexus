import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { step001Initial } from '../../src/sqlite/schema/001_initial.js';
import { step002Questions } from '../../src/sqlite/schema/002_questions.js';
import { SqliteSchemaInspect } from './helpers/SqliteSchemaInspect.js';

describe('SQLite-Schema-Stufe 002_questions', () => {
  let s: SqliteSchemaInspect;
  const p = () => ({ u: SqliteSchemaInspect.USER, now: SqliteSchemaInspect.NOW });
  // Frage in Projekt 1 mit allen Pflichtfeldern; einzelne Spalten überschreibbar
  const insertQuestion = (id: string, number: number, extra: { state?: string; originKind?: string; outcome?: string | null } = {}) =>
    s.run(
      `INSERT INTO Questions (Id, ProjectId, Number, Title, State, OriginKind, OriginName, DecisionOutcome, CreatedAt, CreatedById, UpdatedAt, Rev)
       VALUES (@id, 1, @number, 'Frage', @state, @kind, 'Test', @outcome, @now, @u, @now, 1)`,
      { id, number, state: extra.state ?? 'open', kind: extra.originKind ?? 'human', outcome: extra.outcome ?? null, ...p() },
    );

  beforeEach(() => {
    s = SqliteSchemaInspect.fresh([step001Initial, step002Questions]);
    s.seedBoard();
  });
  afterEach(() => s.dispose());

  it('ergänzt Projects um den Fragenzähler und meldet Schema-Version 2', () => {
    expect(s.columns('Projects').at(-1)).toBe('NextQuestionNumber');
    expect(s.all('SELECT NextQuestionNumber FROM Projects')[0].NextQuestionNumber).toBe(1);
    expect(s.runner.schemaVersion()).toBe(2);
  });

  it('legt alle Fragentabellen mit den Spalten aus SQL Server an', () => {
    expect(s.columns('Questions')).toEqual([
      'Id', 'ProjectId', 'Number', 'Title', 'Description', 'State', 'Occasion', 'OriginKind', 'OriginName', 'OriginSession',
      'DecisionOutcome', 'DecisionOption', 'DecisionText', 'DecisionReason', 'DecidedById', 'DecidedByName', 'DecidedAt',
      'CreatedAt', 'CreatedById', 'UpdatedAt', 'Rev', 'RowVer',
    ]);
    expect(s.columns('QuestionTags')).toEqual(['QuestionId', 'Position', 'Tag']);
    expect(s.columns('QuestionTickets')).toEqual(['QuestionId', 'TicketId']);
    expect(s.columns('QuestionDependencies')).toEqual(['QuestionId', 'Position', 'DependsOnId']);
    expect(s.columns('QuestionResearch')).toEqual(['Id', 'QuestionId', 'At', 'ById', 'ByName', 'Kind', 'Context']);
    expect(s.columns('QuestionOptions')).toEqual(['QuestionId', 'ResearchId', 'OptionId', 'Title', 'Text', 'Pros', 'Cons', 'Recommended']);
    expect(s.columns('QuestionComments')).toEqual(['Id', 'QuestionId', 'AuthorId', 'AuthorName', 'AuthorKind', 'Body', 'CreatedAt']);
    expect(s.columns('QuestionHistory')).toEqual(['Id', 'QuestionId', 'At', 'ById', 'ByName', 'ByKind', 'Type', 'Text']);
  });

  it('legt Indizes und Schlüssel an', () => {
    expect(s.indexes('Questions')).toEqual({ IX_Questions_Project: ['ProjectId', 'State'] });
    expect(s.indexes('QuestionTickets')).toEqual({ IX_QuestionTickets_Ticket: ['TicketId'] });
    expect(s.indexes('QuestionDependencies')).toEqual({ IX_QuestionDeps_DependsOn: ['DependsOnId'] });
    expect(s.indexes('QuestionResearch')).toEqual({ IX_QuestionResearch_Question: ['QuestionId'] });
    expect(s.indexes('QuestionOptions')).toEqual({ IX_QuestionOptions_Research: ['ResearchId'] });
    expect(s.indexes('QuestionComments')).toEqual({ IX_QuestionComments_Question: ['QuestionId'] });
    expect(s.indexes('QuestionHistory')).toEqual({ IX_QuestionHistory_Question: ['QuestionId'] });
    expect(s.uniques('Questions')).toEqual([['ProjectId', 'Number']]);
    expect(s.primaryKey('QuestionTags')).toEqual(['QuestionId', 'Tag']);
    expect(s.primaryKey('QuestionTickets')).toEqual(['QuestionId', 'TicketId']);
    expect(s.primaryKey('QuestionDependencies')).toEqual(['QuestionId', 'DependsOnId']);
    expect(s.primaryKey('QuestionOptions')).toEqual(['QuestionId', 'OptionId']);
  });

  it('übernimmt Fremdschlüssel und Kaskaden wie im Original', () => {
    expect(s.foreignKeys('Questions')).toEqual([
      { from: 'CreatedById', table: 'Users', to: 'Id', onDelete: 'NO ACTION' },
      { from: 'DecidedById', table: 'Users', to: 'Id', onDelete: 'NO ACTION' },
      { from: 'ProjectId', table: 'Projects', to: 'Id', onDelete: 'NO ACTION' },
    ]);
    expect(s.foreignKeys('QuestionTickets')).toEqual([
      { from: 'QuestionId', table: 'Questions', to: 'Id', onDelete: 'CASCADE' },
      { from: 'TicketId', table: 'Tickets', to: 'Id', onDelete: 'NO ACTION' },
    ]);
    expect(s.foreignKeys('QuestionDependencies')).toEqual([{ from: 'QuestionId', table: 'Questions', to: 'Id', onDelete: 'CASCADE' }]);
    expect(s.foreignKeys('QuestionOptions')).toEqual([{ from: 'ResearchId', table: 'QuestionResearch', to: 'Id', onDelete: 'CASCADE' }]);
    for (const table of ['QuestionTags', 'QuestionResearch', 'QuestionComments', 'QuestionHistory']) {
      expect(s.foreignKeys(table).find((f) => f.from === 'QuestionId'), table).toEqual({ from: 'QuestionId', table: 'Questions', to: 'Id', onDelete: 'CASCADE' });
    }
    expect(s.foreignKeyCheck()).toEqual([]);
  });

  it('lässt nur gültige Werte in den CHECK-Spalten zu', () => {
    expect(() => insertQuestion('AIH-F001', 1, { state: 'vielleicht' })).toThrow(/CK_Questions_State/);
    expect(() => insertQuestion('AIH-F001', 1, { originKind: 'robot' })).toThrow(/CK_Questions_OriginKind/);
    expect(() => insertQuestion('AIH-F001', 1, { outcome: 'maybe' })).toThrow(/CK_Questions_Outcome/);
    insertQuestion('AIH-F001', 1, { state: 'answered', outcome: 'answered' });
    insertQuestion('AIH-F002', 2);
    expect(() => s.run("INSERT INTO QuestionResearch (QuestionId, At, ById, ByName, Kind) VALUES ('AIH-F001', @now, @u, 'T', 'x')", p())).toThrow(/CK_QuestionResearch_Kind/);
    expect(() => s.run("INSERT INTO QuestionComments (QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt) VALUES ('AIH-F001', @u, 'T', 'x', 'b', @now)", p())).toThrow(/CK_QuestionComments_Kind/);
    expect(() => s.run("INSERT INTO QuestionHistory (QuestionId, At, ById, ByName, ByKind, Type, Text) VALUES ('AIH-F001', @now, @u, 'T', 'x', 'created', 't')", p())).toThrow(/CK_QuestionHistory_Kind/);
    s.run("INSERT INTO QuestionHistory (QuestionId, At, ById, ByName, ByKind, Type, Text) VALUES ('AIH-F001', @now, @u, 'T', NULL, 'created', 't')", p());
    expect(() => insertQuestion('AIH-F003', 2)).toThrow(/UNIQUE constraint failed: Questions\.ProjectId, Questions\.Number/);
  });

  it('löscht Tags, Recherchen samt Optionen, Kommentare und Verlauf mit der Frage', () => {
    insertQuestion('AIH-F001', 1);
    s.insertTicket('AIH-0001', 1);
    s.run("INSERT INTO QuestionTags (QuestionId, Position, Tag) VALUES ('AIH-F001', 0, 'db')");
    s.run("INSERT INTO QuestionTickets (QuestionId, TicketId) VALUES ('AIH-F001', 'AIH-0001')");
    s.run("INSERT INTO QuestionDependencies (QuestionId, Position, DependsOnId) VALUES ('AIH-F001', 0, 'AIH-0001')");
    s.run("INSERT INTO QuestionResearch (QuestionId, At, ById, ByName, Kind) VALUES ('AIH-F001', @now, @u, 'T', 'session')", p());
    s.run("INSERT INTO QuestionOptions (QuestionId, ResearchId, OptionId, Title, Text, Recommended) VALUES ('AIH-F001', 1, 'O1', 'Option', 'Text', 1)");
    s.run("INSERT INTO QuestionComments (QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt) VALUES ('AIH-F001', @u, 'T', 'human', 'b', @now)", p());
    s.run("INSERT INTO QuestionHistory (QuestionId, At, ById, ByName, Type, Text) VALUES ('AIH-F001', @now, @u, 'T', 'created', 't')", p());
    expect(s.all('SELECT Pros, Cons FROM QuestionOptions')[0]).toEqual({ Pros: '[]', Cons: '[]' });
    expect(() => s.run("DELETE FROM Tickets WHERE Id = 'AIH-0001'")).toThrow(/FOREIGN KEY constraint failed/);
    s.run("DELETE FROM Questions WHERE Id = 'AIH-F001'");
    for (const table of ['QuestionTags', 'QuestionTickets', 'QuestionDependencies', 'QuestionResearch', 'QuestionOptions', 'QuestionComments', 'QuestionHistory']) {
      expect(s.all(`SELECT COUNT(*) AS N FROM ${table}`)[0].N, table).toBe(0);
    }
  });
});
