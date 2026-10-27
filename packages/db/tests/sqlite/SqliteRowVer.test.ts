import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { SQLITE_SCHEMA_STEPS } from '../../src/sqlite/schema/index.js';
import { SqliteRunner } from '../../src/sqlite/SqliteRunner.js';
import { SqliteSchemaInspect } from './helpers/SqliteSchemaInspect.js';

describe('SQLite-Schema-Stufe 005_rowver (RowVer-Zähler per Trigger)', () => {
  let s: SqliteSchemaInspect;
  const p = () => ({ u: SqliteSchemaInspect.USER, now: SqliteSchemaInspect.NOW });
  const rowVer = (table: string, id: string | number) => s.all<{ V: number }>(`SELECT RowVer AS V FROM ${table} WHERE Id = @id`, { id })[0].V;
  const counter = () => s.all<{ V: number }>('SELECT Value AS V FROM ChangeCounter')[0].V;

  beforeEach(() => {
    s = SqliteSchemaInspect.fresh(SQLITE_SCHEMA_STEPS);
    s.seedBoard(); // Board 1 bekommt RowVer 1 (Users/Projects haben keinen Trigger)
  });
  afterEach(() => s.dispose());

  it('legt den Zähler mit genau einer Zeile und je Tabelle zwei Trigger an', () => {
    expect(s.columns('ChangeCounter')).toEqual(['Id', 'Value']);
    expect(() => s.run('INSERT INTO ChangeCounter (Id, Value) VALUES (2, 0)')).toThrow(/CK_ChangeCounter_Single/);
    expect(s.triggers()).toEqual([
      'TR_Boards_RowVer_Insert', 'TR_Boards_RowVer_Update', 'TR_Comments_RowVer_Insert', 'TR_Comments_RowVer_Update',
      'TR_Notes_RowVer_Insert', 'TR_Notes_RowVer_Update', 'TR_Questions_RowVer_Insert', 'TR_Questions_RowVer_Update',
      'TR_Tickets_RowVer_Insert', 'TR_Tickets_RowVer_Update',
    ]);
  });

  it('erhöht RowVer bei Insert und Update streng monoton über alle fünf Tabellen', () => {
    const seen = [rowVer('Boards', 1)];
    s.insertTicket('AIH-0001', 1);
    seen.push(rowVer('Tickets', 'AIH-0001'));
    s.run("INSERT INTO Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt) VALUES ('AIH-0001', @u, 'T', 'b', @now)", p());
    seen.push(rowVer('Comments', 1));
    s.run("INSERT INTO Notes (BoardId, AuthorId, AuthorName, Body, State, CreatedAt) VALUES (1, @u, 'T', 'b', 'open', @now)", p());
    seen.push(rowVer('Notes', 1));
    s.run(`INSERT INTO Questions (Id, ProjectId, Number, Title, State, OriginKind, OriginName, CreatedAt, CreatedById, UpdatedAt, Rev)
           VALUES ('AIH-F001', 1, 1, 'Frage', 'open', 'human', 'T', @now, @u, @now, 1)`, p());
    seen.push(rowVer('Questions', 'AIH-F001'));
    s.run("UPDATE Boards SET Name = 'B2' WHERE Id = 1");
    seen.push(rowVer('Boards', 1));
    s.run("UPDATE Tickets SET Title = 'T2' WHERE Id = 'AIH-0001'");
    seen.push(rowVer('Tickets', 'AIH-0001'));
    s.run('UPDATE Comments SET ReadByAgentAt = @now WHERE Id = 1', { now: SqliteSchemaInspect.NOW });
    seen.push(rowVer('Comments', 1));
    s.run("UPDATE Notes SET State = 'taken' WHERE Id = 1");
    seen.push(rowVer('Notes', 1));
    s.run("UPDATE Questions SET Title = 'F2' WHERE Id = 'AIH-F001'");
    seen.push(rowVer('Questions', 'AIH-F001'));
    expect(seen).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    expect(counter()).toBe(10);
  });

  it('gibt bei einem Update mehrerer Zeilen jeder Zeile einen eigenen Wert', () => {
    s.insertTicket('AIH-0001', 1);
    s.insertTicket('AIH-0002', 2);
    s.run("UPDATE Tickets SET Title = Title || '!'");
    expect(s.all('SELECT Id, RowVer FROM Tickets ORDER BY Id')).toEqual([{ Id: 'AIH-0001', RowVer: 4 }, { Id: 'AIH-0002', RowVer: 5 }]);
    expect(counter()).toBe(5);
  });

  it('löst beim Setzen von RowVer selbst keine Endlosschleife aus, auch mit recursive_triggers', () => {
    s.insertTicket('AIH-0001', 1);
    s.run("UPDATE Tickets SET RowVer = 500 WHERE Id = 'AIH-0001'");
    expect(rowVer('Tickets', 'AIH-0001')).toBe(500);
    expect(counter()).toBe(2);
    s.run('PRAGMA recursive_triggers = ON');
    s.run("UPDATE Boards SET Name = 'B3' WHERE Id = 1");
    expect(rowVer('Boards', 1)).toBe(3);
    expect(counter()).toBe(3);
  });

  it('zählt für SqliteRunner nur die direkt geänderten Zeilen (optimistische Prüfungen bleiben richtig)', async () => {
    s.insertTicket('AIH-0001', 1);
    const sql = new SqliteRunner(s.db);
    const result = await sql.query('UPDATE Tickets SET Title = @title, Rev = Rev + 1 WHERE Id = @id AND Rev = @rev', { title: 'Neu', id: 'AIH-0001', rev: 1 });
    expect(result.affected).toBe(1);
    expect((await sql.query('UPDATE Tickets SET Title = @title WHERE Id = @id AND Rev = @rev', { title: 'Alt', id: 'AIH-0001', rev: 1 })).affected).toBe(0);
  });
});
