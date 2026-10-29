-- Erweiterung „dispatch-runs“ des Boards: Läufe des Dispatchers für die Auswertung (Dashboard). In AiHarness_Test und in
-- AiHarness ausführen, nach 003. Vorher im Abfragefenster die richtige Datenbank wählen. Ein einziger Batch in einer
-- Transaktion: fehlt Version 3 oder ist die Erweiterung schon eingespielt, bricht das Skript ab und ändert nichts.
--
-- Die Schema-Version bleibt 3: ältere Harness-Stände prüfen MAX(board.SchemaVersion) und würden eine 4 ablehnen. Rein
-- additive Erweiterungen tragen sich stattdessen in board.SchemaFeatures ein; neuere Stände verlangen den Eintrag.

SET XACT_ABORT ON;
BEGIN TRANSACTION;

IF NOT EXISTS (SELECT 1 FROM board.SchemaVersion WHERE Version = 3)
  THROW 50001, N'Schema-Version 3 fehlt: zuerst 003_harness.sql ausführen.', 1;
IF OBJECT_ID(N'board.DispatchRuns') IS NOT NULL
  THROW 50002, N'Erweiterung dispatch-runs ist bereits eingespielt.', 1;

-- Eingespielte additive Erweiterungen (ältere Harness-Stände lesen die Tabelle nicht)
CREATE TABLE board.SchemaFeatures (
  Name      varchar(50)   NOT NULL CONSTRAINT PK_SchemaFeatures PRIMARY KEY,
  Script    nvarchar(100) NOT NULL,
  AppliedAt datetime2(3)  NOT NULL CONSTRAINT DF_SchemaFeatures_AppliedAt DEFAULT SYSUTCDATETIME()
);

-- Ein ausgewerteter Lauf je Zeile; kein Fremdschlüssel auf Tickets (Recherchen haben Fragen-IDs, die Statistik
-- überlebt gelöschte Tickets)
CREATE TABLE board.DispatchRuns (
  Id               nvarchar(100) NOT NULL CONSTRAINT PK_DispatchRuns PRIMARY KEY,
  Kind             varchar(10)   NOT NULL CONSTRAINT CK_DispatchRuns_Kind CHECK (Kind IN ('work', 'merge', 'research')),
  SubjectId        nvarchar(20)  NOT NULL,
  ProjectId        int           NOT NULL,
  BoardId          int           NULL,
  Agent            nvarchar(100) NOT NULL,
  Attempt          int           NULL,
  Profile          nvarchar(50)  NOT NULL,
  EscalatedFrom    nvarchar(50)  NULL,
  RequestedModel   nvarchar(100) NULL,
  Model            nvarchar(100) NULL,
  Effort           nvarchar(20)  NULL,
  StartedAt        datetime2(3)  NOT NULL,
  EndedAt          datetime2(3)  NULL,
  DurationMs       int           NULL,
  NumTurns         int           NULL,
  ExitCode         int           NULL,
  TimedOut         bit           NOT NULL CONSTRAINT DF_DispatchRuns_TimedOut DEFAULT 0,
  Outcome          varchar(30)   NOT NULL,
  AbortCategory    varchar(30)   NULL,
  AbortReason      nvarchar(500) NULL,
  CostUsd          decimal(12,6) NULL,
  InputTokens      bigint        NULL,
  OutputTokens     bigint        NULL,
  CacheReadTokens  bigint        NULL,
  CacheWriteTokens bigint        NULL,
  ModelUsage       nvarchar(max) NULL,
  Source           varchar(10)   NOT NULL CONSTRAINT CK_DispatchRuns_Source CHECK (Source IN ('dispatcher', 'comments')),
  RecordedAt       datetime2(3)  NOT NULL CONSTRAINT DF_DispatchRuns_RecordedAt DEFAULT SYSUTCDATETIME()
);
CREATE INDEX IX_DispatchRuns_Project_Started ON board.DispatchRuns(ProjectId, StartedAt) INCLUDE (BoardId, Kind, Outcome, Profile, Model, CostUsd);
CREATE INDEX IX_DispatchRuns_Subject ON board.DispatchRuns(SubjectId);

-- Statuswechsel nach Zeit (Durchsatz, Durchlaufzeiten); ohne Filter, damit ältere Stände ohne besondere SET-Optionen schreiben
CREATE INDEX IX_History_Field_At ON board.History(Field, At) INCLUDE (TicketId, ToValue);

INSERT INTO board.SchemaFeatures (Name, Script) VALUES ('dispatch-runs', N'004_dispatch_runs.sql');
COMMIT TRANSACTION;
GO
