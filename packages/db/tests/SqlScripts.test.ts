import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

// Inhalt eines SQL-Skripts aus packages/db/sql
const script = (name: string) => readFileSync(fileURLToPath(new URL(`../sql/${name}`, import.meta.url)), 'utf8');

describe('SQL-Skript 000_create_databases.sql', () => {
  const sql = script('000_create_databases.sql');

  it('gibt dem Login in beiden Datenbanken DDL, damit der Harness Erweiterungen selbst einspielen kann', () => {
    expect(sql.match(/^ALTER ROLE db_ddladmin ADD MEMBER \[<LOGIN>\];$/gm)).toHaveLength(2);
    expect(sql.match(/^ALTER ROLE db_datawriter ADD MEMBER \[<LOGIN>\];$/gm)).toHaveLength(2);
  });

  it('löscht nichts', () => {
    expect(sql).not.toMatch(/\bDROP\b|\bTRUNCATE\b|DELETE FROM/i);
  });
});

describe('SQL-Skript 002_questions.sql', () => {
  const sql = script('002_questions.sql');

  it('erweitert Projects um den Fragenzähler und setzt Schema-Version 2', () => {
    expect(sql).toContain('ALTER TABLE board.Projects ADD NextQuestionNumber int NOT NULL CONSTRAINT DF_Projects_NextQuestion DEFAULT 1;');
    expect(sql).toContain('INSERT INTO board.SchemaVersion (Version) VALUES (2);');
  });

  it('legt alle Fragentabellen an', () => {
    for (const table of ['Questions', 'QuestionTags', 'QuestionTickets', 'QuestionDependencies', 'QuestionResearch', 'QuestionOptions', 'QuestionComments', 'QuestionHistory']) {
      expect(sql, table).toContain(`CREATE TABLE board.${table} (`);
    }
    expect(sql).toContain('CONSTRAINT UQ_Questions_Number UNIQUE (ProjectId, Number)');
  });

  it('bricht ab, wenn Version 1 fehlt oder Version 2 schon da ist, und löscht nichts', () => {
    expect(sql).toContain('SET XACT_ABORT ON;');
    expect(sql).toMatch(/WHERE Version = 1\)\s+THROW 50001/);
    expect(sql).toMatch(/WHERE Version >= 2\)\s+THROW 50002/);
    expect(sql).toMatch(/COMMIT TRANSACTION;\s*GO\s*$/);
    expect(sql).not.toMatch(/\bDROP\b|\bTRUNCATE\b|DELETE FROM/i);
    expect(sql.match(/^GO\s*$/gm)).toHaveLength(1);
  });
});

describe('SQL-Skript 003_harness.sql', () => {
  const sql = script('003_harness.sql');

  it('ergänzt Tickets um Spec, Aufgabe, Branch, Worktree und die JSON-Listen', () => {
    for (const column of [
      'Spec     nvarchar(200) NULL', 'SpecTask nvarchar(20)  NULL', 'Branch   nvarchar(400) NULL', 'Worktree nvarchar(500) NULL',
      "Touches  nvarchar(max) NOT NULL CONSTRAINT DF_Tickets_Touches DEFAULT N'[]'",
      "Consumes nvarchar(max) NOT NULL CONSTRAINT DF_Tickets_Consumes DEFAULT N'[]'",
      "Produces nvarchar(max) NOT NULL CONSTRAINT DF_Tickets_Produces DEFAULT N'[]'",
    ]) {
      expect(sql, column).toContain(column);
    }
    expect(sql).toContain('ALTER TABLE board.Comments ADD ReadByAgentAt datetime2(3) NULL;');
    expect(sql).toContain('INSERT INTO board.SchemaVersion (Version) VALUES (3);');
  });

  it('legt Schritte und Sessions je Ticket mit Kaskade an', () => {
    expect(sql).toContain('CREATE TABLE board.TicketSteps (');
    expect(sql).toContain('CONSTRAINT PK_TicketSteps PRIMARY KEY (TicketId, Position)');
    expect(sql).toContain('CREATE TABLE board.TicketSessions (');
    expect(sql).toContain('CONSTRAINT PK_TicketSessions PRIMARY KEY (TicketId, SessionId)');
    expect(sql.match(/REFERENCES board\.Tickets\(Id\) ON DELETE CASCADE/g)).toHaveLength(2);
  });

  it('bricht ab, wenn Version 2 fehlt oder Version 3 schon da ist, und löscht nichts', () => {
    expect(sql).toContain('SET XACT_ABORT ON;');
    expect(sql).toMatch(/WHERE Version = 2\)\s+THROW 50001/);
    expect(sql).toMatch(/WHERE Version >= 3\)\s+THROW 50002/);
    expect(sql).toMatch(/COMMIT TRANSACTION;\s*GO\s*$/);
    expect(sql).not.toMatch(/\bDROP\b|\bTRUNCATE\b|DELETE FROM|\bUSE\b/i);
    expect(sql.match(/^GO\s*$/gm)).toHaveLength(1);
  });

  it('verweist im Batch nicht auf die neu angelegten Ticket-Spalten (sonst scheitert die Übersetzung)', () => {
    const afterAlter = sql.slice(sql.indexOf('ALTER TABLE board.Tickets ADD') + 1);
    const rest = afterAlter.slice(afterAlter.indexOf(';') + 1);
    expect(rest).not.toMatch(/\b(SpecTask|Worktree|Touches|Consumes|Produces)\b/);
  });
});

describe('SQL-Skript 004_dispatch_runs.sql', () => {
  const sql = script('004_dispatch_runs.sql');

  it('legt Lauf-Tabelle, Indizes und die Erweiterungs-Tabelle an', () => {
    expect(sql).toContain('CREATE TABLE board.DispatchRuns (');
    expect(sql).toContain('CREATE TABLE board.SchemaFeatures (');
    expect(sql).toContain("CONSTRAINT CK_DispatchRuns_Kind CHECK (Kind IN ('work', 'merge', 'research'))");
    expect(sql).toContain("CONSTRAINT CK_DispatchRuns_Source CHECK (Source IN ('dispatcher', 'comments'))");
    for (const index of ['IX_DispatchRuns_Project_Started', 'IX_DispatchRuns_Subject', 'IX_History_Field_At']) expect(sql, index).toContain(`CREATE INDEX ${index} `);
    expect(sql).toContain("INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('dispatch-runs', N'004_dispatch_runs.sql');");
  });

  it('erhöht die Schema-Version nicht (ältere Stände laufen weiter) und hat keinen Fremdschlüssel auf Tickets', () => {
    expect(sql).not.toMatch(/INSERT INTO board\.SchemaVersion/);
    expect(sql).not.toMatch(/REFERENCES/);
    // Gefilterte Indizes verlangen besondere SET-Optionen von jedem Schreiber – auch von älteren Ständen
    expect(sql).not.toMatch(/CREATE INDEX[^;]*\bWHERE\b/);
  });

  it('bricht ab, wenn Version 3 fehlt oder die Erweiterung schon da ist, und löscht nichts', () => {
    expect(sql).toContain('SET XACT_ABORT ON;');
    expect(sql).toMatch(/WHERE Version = 3\)\s+THROW 50001/);
    expect(sql).toMatch(/OBJECT_ID\(N'board\.DispatchRuns'\) IS NOT NULL\s+THROW 50002/);
    expect(sql).toMatch(/COMMIT TRANSACTION;\s*GO\s*$/);
    expect(sql).not.toMatch(/\bDROP\b|\bTRUNCATE\b|DELETE FROM|\bUSE\b/i);
    expect(sql.match(/^GO\s*$/gm)).toHaveLength(1);
  });
});

describe('SQL-Skript 005_note_structure.sql', () => {
  const sql = script('005_note_structure.sql');

  it('ergänzt Notizen um Titel und Zähler mit Defaults und trägt die Erweiterung ein', () => {
    expect(sql).toContain("ALTER TABLE board.Notes ADD Title nvarchar(200) NOT NULL CONSTRAINT DF_Notes_Title DEFAULT N'';");
    expect(sql).toContain('ALTER TABLE board.Notes ADD SeenCount int NOT NULL CONSTRAINT DF_Notes_SeenCount DEFAULT 1;');
    expect(sql).toContain("ALTER TABLE board.Notes ADD SeenIn nvarchar(1000) NOT NULL CONSTRAINT DF_Notes_SeenIn DEFAULT N'';");
    expect(sql).toContain("INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('note-structure', N'005_note_structure.sql');");
    expect(sql).not.toMatch(/INSERT INTO board\.SchemaVersion/);
  });

  it('bricht ab, wenn dispatch-runs fehlt oder die Erweiterung schon da ist, und löscht nichts', () => {
    expect(sql).toContain('SET XACT_ABORT ON;');
    expect(sql).toMatch(/Name = 'dispatch-runs'\)\s+THROW 50001/);
    expect(sql).toMatch(/COL_LENGTH\(N'board\.Notes', N'Title'\) IS NOT NULL\s+THROW 50002/);
    expect(sql).toMatch(/COMMIT TRANSACTION;\s*GO\s*$/);
    expect(sql).not.toMatch(/\bDROP\b|\bTRUNCATE\b|DELETE FROM|\bUSE\b/i);
    expect(sql.match(/^GO\s*$/gm)).toHaveLength(1);
  });
});

describe('SQL-Skript 006_dispatch_run_details.sql', () => {
  const sql = script('006_dispatch_run_details.sql');

  it('ergänzt die Lauf-Spalten für das Dashboard und trägt die Erweiterung ein', () => {
    for (const column of [
      'ALTER TABLE board.DispatchRuns ADD MaxTurns int NULL;',
      'ALTER TABLE board.DispatchRuns ADD TimeoutMinutes int NULL;',
      'ALTER TABLE board.DispatchRuns ADD RuleIndex int NULL;',
      'ALTER TABLE board.DispatchRuns ADD ConfigHash varchar(64) NULL;',
      'ALTER TABLE board.DispatchRuns ADD DeniedCount int NULL;',
      'ALTER TABLE board.DispatchRuns ADD DeniedTools nvarchar(500) NULL;',
    ]) {
      expect(sql, column).toContain(column);
    }
    expect(sql).toContain("CONSTRAINT CK_DispatchRuns_ProfileReason CHECK (ProfileReason IN ('default', 'label', 'plan', 'rule', 'setting'))");
    expect(sql).toContain("INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('dispatch-run-details', N'006_dispatch_run_details.sql');");
    expect(sql).not.toMatch(/INSERT INTO board\.SchemaVersion/);
  });

  it('legt die Tabelle der Konfig-Versionen mit Indizes an', () => {
    expect(sql).toContain('CREATE TABLE board.DispatchConfigVersions (');
    expect(sql).toContain('CONSTRAINT PK_DispatchConfigVersions PRIMARY KEY');
    expect(sql).toContain('CREATE INDEX IX_DispatchConfigVersions_FirstSeen ');
    // Index auf der neuen Spalte nur als Zeichenkette: im selben Batch kennt der Übersetzer ConfigHash noch nicht
    expect(sql).toContain("EXEC(N'CREATE INDEX IX_DispatchRuns_Config ON board.DispatchRuns(ConfigHash)');");
  });

  it('bricht ab, wenn note-structure fehlt oder die Erweiterung schon da ist, und löscht nichts', () => {
    expect(sql).toContain('SET XACT_ABORT ON;');
    expect(sql).toMatch(/Name = 'note-structure'\)\s+THROW 50001/);
    expect(sql).toMatch(/COL_LENGTH\(N'board\.DispatchRuns', N'MaxTurns'\) IS NOT NULL\s+THROW 50002/);
    expect(sql).toMatch(/COMMIT TRANSACTION;\s*GO\s*$/);
    expect(sql).not.toMatch(/\bDROP\b|\bTRUNCATE\b|DELETE FROM|\bUSE\b/i);
    expect(sql.match(/^GO\s*$/gm)).toHaveLength(1);
  });
});

describe('SQL-Skript 007_target_branch.sql', () => {
  const sql = script('007_target_branch.sql');

  it('ergänzt Tickets um TargetBranch und trägt die Erweiterung ein', () => {
    expect(sql).toContain('ALTER TABLE board.Tickets ADD TargetBranch nvarchar(400) NULL;');
    expect(sql).toContain("INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('target-branch', N'007_target_branch.sql');");
    expect(sql).not.toContain('INSERT INTO board.SchemaVersion');
  });

  it('bricht ab ohne dispatch-runs oder bei doppeltem Einspielen und löscht nichts', () => {
    expect(sql).toContain('SET XACT_ABORT ON;');
    expect(sql).toMatch(/OBJECT_ID\(N'board\.SchemaFeatures'\) IS NULL\s+THROW 50001/);
    expect(sql).toMatch(/Name = 'target-branch'\)\s+THROW 50002/);
    expect(sql).toMatch(/COMMIT TRANSACTION;\s*GO\s*$/);
    expect(sql).not.toMatch(/\bDROP\b|\bTRUNCATE\b|DELETE FROM|\bUSE\b/i);
    expect(sql.match(/^GO\s*$/gm)).toHaveLength(1);
  });
});
