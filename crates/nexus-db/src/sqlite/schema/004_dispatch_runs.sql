
-- Eingespielte additive Erweiterungen; den Eintrag „dispatch-runs“ schreibt der Schema-Runner nach dieser Stufe
CREATE TABLE SchemaFeatures (
  Name      TEXT NOT NULL CONSTRAINT PK_SchemaFeatures PRIMARY KEY,
  Script    TEXT NOT NULL,
  AppliedAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Ein ausgewerteter Lauf je Zeile; kein Fremdschlüssel auf Tickets (Recherchen haben Fragen-IDs, die Statistik
-- überlebt gelöschte Tickets)
CREATE TABLE DispatchRuns (
  Id               TEXT    NOT NULL CONSTRAINT PK_DispatchRuns PRIMARY KEY,
  Kind             TEXT    NOT NULL CONSTRAINT CK_DispatchRuns_Kind CHECK (Kind IN ('work', 'merge', 'research')),
  SubjectId        TEXT    NOT NULL,
  ProjectId        INTEGER NOT NULL,
  BoardId          INTEGER NULL,
  Agent            TEXT    NOT NULL,
  Attempt          INTEGER NULL,
  Profile          TEXT    NOT NULL,
  EscalatedFrom    TEXT    NULL,
  RequestedModel   TEXT    NULL,
  Model            TEXT    NULL,
  Effort           TEXT    NULL,
  StartedAt        TEXT    NOT NULL,
  EndedAt          TEXT    NULL,
  DurationMs       INTEGER NULL,
  NumTurns         INTEGER NULL,
  ExitCode         INTEGER NULL,
  TimedOut         INTEGER NOT NULL DEFAULT 0,
  Outcome          TEXT    NOT NULL,
  AbortCategory    TEXT    NULL,
  AbortReason      TEXT    NULL,
  CostUsd          REAL    NULL,
  InputTokens      INTEGER NULL,
  OutputTokens     INTEGER NULL,
  CacheReadTokens  INTEGER NULL,
  CacheWriteTokens INTEGER NULL,
  ModelUsage       TEXT    NULL,
  Source           TEXT    NOT NULL CONSTRAINT CK_DispatchRuns_Source CHECK (Source IN ('dispatcher', 'comments')),
  RecordedAt       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
-- SQLite kennt kein INCLUDE: die Indizes enthalten nur die Schlüsselspalten
CREATE INDEX IX_DispatchRuns_Project_Started ON DispatchRuns(ProjectId, StartedAt);
CREATE INDEX IX_DispatchRuns_Subject ON DispatchRuns(SubjectId);

-- Statuswechsel nach Zeit (Durchsatz, Durchlaufzeiten)
CREATE INDEX IX_History_Field_At ON History(Field, At);
