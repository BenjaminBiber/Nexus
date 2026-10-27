import type { SqliteSchemaStep } from './SqliteSchemaStep.js';

// Schema-Version 1 (entspricht packages/db/sql/001_initial.sql): Nutzer, Projekte, Boards, Tickets, Labels,
// Voraussetzungen, Kommentare, Verlauf, Notizen, Löschungen
export const step001Initial: SqliteSchemaStep = {
  id: '001_initial',
  version: 1,
  sql: `
CREATE TABLE SchemaVersion (
  Version   INTEGER NOT NULL CONSTRAINT PK_SchemaVersion PRIMARY KEY,
  AppliedAt TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE Users (
  Id        TEXT NOT NULL CONSTRAINT PK_Users PRIMARY KEY,
  Name      TEXT NOT NULL,
  CreatedAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- AUTOINCREMENT: Projekt- und Board-Ids stehen in Konfigurationen und dürfen nach dem Löschen nicht wiederkommen
CREATE TABLE Projects (
  Id               INTEGER PRIMARY KEY AUTOINCREMENT,
  "Key"            TEXT    NOT NULL CONSTRAINT UQ_Projects_Key UNIQUE,
  Name             TEXT    NOT NULL,
  NextTicketNumber INTEGER NOT NULL DEFAULT 1,
  CreatedAt        TEXT    NOT NULL,
  CreatedBy        TEXT    NOT NULL CONSTRAINT FK_Projects_Users REFERENCES Users(Id)
);

CREATE TABLE Boards (
  Id        INTEGER PRIMARY KEY AUTOINCREMENT,
  ProjectId INTEGER NOT NULL CONSTRAINT FK_Boards_Projects REFERENCES Projects(Id),
  OwnerId   TEXT    NOT NULL CONSTRAINT FK_Boards_Users REFERENCES Users(Id),
  Name      TEXT    NOT NULL,
  RepoPath  TEXT    NULL,
  CreatedAt TEXT    NOT NULL,
  RowVer    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IX_Boards_Project ON Boards(ProjectId);

CREATE TABLE Tickets (
  Id              TEXT    NOT NULL CONSTRAINT PK_Tickets PRIMARY KEY,
  ProjectId       INTEGER NOT NULL CONSTRAINT FK_Tickets_Projects REFERENCES Projects(Id),
  BoardId         INTEGER NOT NULL CONSTRAINT FK_Tickets_Boards REFERENCES Boards(Id),
  Number          INTEGER NOT NULL,
  Title           TEXT    NOT NULL,
  Description     TEXT    NOT NULL DEFAULT '',
  Status          TEXT    NOT NULL CONSTRAINT CK_Tickets_Status CHECK (Status IN ('backlog','in_planning','planned','in_progress','blocked','done','not_needed')),
  Priority        TEXT    NOT NULL CONSTRAINT CK_Tickets_Priority CHECK (Priority IN ('low','normal','high','urgent')),
  SortOrder       INTEGER NOT NULL,
  Assignee        TEXT    NULL,
  NeedsPlanning   INTEGER NOT NULL,
  Acceptance      TEXT    NOT NULL DEFAULT '',
  "Plan"          TEXT    NOT NULL DEFAULT '',
  Handbook        TEXT    NOT NULL DEFAULT '',
  Diagram         TEXT    NOT NULL DEFAULT '',
  BlockedReason   TEXT    NULL,
  PlannedAt       TEXT    NULL,
  PlannedBy       TEXT    NULL,
  PlanningClaimBy TEXT    NULL,
  PlanningClaimAt TEXT    NULL,
  CreatedAt       TEXT    NOT NULL,
  CreatedById     TEXT    NOT NULL CONSTRAINT FK_Tickets_Users REFERENCES Users(Id),
  CreatedByName   TEXT    NOT NULL,
  UpdatedAt       TEXT    NOT NULL,
  Rev             INTEGER NOT NULL,
  RowVer          INTEGER NOT NULL DEFAULT 0,
  CONSTRAINT UQ_Tickets_Number UNIQUE (ProjectId, Number)
);
CREATE INDEX IX_Tickets_Board ON Tickets(BoardId, Status);

CREATE TABLE TicketLabels (
  TicketId TEXT    NOT NULL CONSTRAINT FK_TicketLabels_Tickets REFERENCES Tickets(Id) ON DELETE CASCADE,
  Position INTEGER NOT NULL,
  Label    TEXT    NOT NULL,
  CONSTRAINT PK_TicketLabels PRIMARY KEY (TicketId, Label)
);

-- Zweiter Fremdschlüssel ohne CASCADE wie in SQL Server: der Dienst löscht Verweise auf ein Ticket vor dem Ticket selbst
CREATE TABLE TicketDependencies (
  TicketId    TEXT    NOT NULL CONSTRAINT FK_TicketDeps_Tickets REFERENCES Tickets(Id) ON DELETE CASCADE,
  Position    INTEGER NOT NULL,
  DependsOnId TEXT    NOT NULL CONSTRAINT FK_TicketDeps_DependsOn REFERENCES Tickets(Id),
  CONSTRAINT PK_TicketDependencies PRIMARY KEY (TicketId, DependsOnId)
);
CREATE INDEX IX_TicketDeps_DependsOn ON TicketDependencies(DependsOnId);

CREATE TABLE Comments (
  Id         INTEGER PRIMARY KEY,
  TicketId   TEXT    NOT NULL CONSTRAINT FK_Comments_Tickets REFERENCES Tickets(Id) ON DELETE CASCADE,
  AuthorId   TEXT    NOT NULL CONSTRAINT FK_Comments_Users REFERENCES Users(Id),
  AuthorName TEXT    NOT NULL,
  Body       TEXT    NOT NULL,
  CreatedAt  TEXT    NOT NULL,
  RowVer     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IX_Comments_Ticket ON Comments(TicketId);

CREATE TABLE History (
  Id        INTEGER PRIMARY KEY,
  TicketId  TEXT NOT NULL CONSTRAINT FK_History_Tickets REFERENCES Tickets(Id) ON DELETE CASCADE,
  At        TEXT NOT NULL,
  ById      TEXT NOT NULL CONSTRAINT FK_History_Users REFERENCES Users(Id),
  ByName    TEXT NOT NULL,
  Field     TEXT NOT NULL,
  FromValue TEXT NULL,
  ToValue   TEXT NULL,
  Note      TEXT NULL
);
CREATE INDEX IX_History_Ticket ON History(TicketId);

CREATE TABLE Notes (
  Id         INTEGER PRIMARY KEY,
  BoardId    INTEGER NOT NULL CONSTRAINT FK_Notes_Boards REFERENCES Boards(Id) ON DELETE CASCADE,
  AuthorId   TEXT    NOT NULL CONSTRAINT FK_Notes_Users REFERENCES Users(Id),
  AuthorName TEXT    NOT NULL,
  Body       TEXT    NOT NULL,
  Source     TEXT    NULL,
  Labels     TEXT    NOT NULL DEFAULT '',
  State      TEXT    NOT NULL CONSTRAINT CK_Notes_State CHECK (State IN ('open','taken','dropped')),
  TicketId   TEXT    NULL,
  Reason     TEXT    NULL,
  CreatedAt  TEXT    NOT NULL,
  RowVer     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IX_Notes_Board ON Notes(BoardId, State);

CREATE TABLE Deletions (
  Id          INTEGER PRIMARY KEY,
  At          TEXT NOT NULL,
  ById        TEXT NOT NULL CONSTRAINT FK_Deletions_Users REFERENCES Users(Id),
  ByName      TEXT NOT NULL,
  TicketId    TEXT NOT NULL,
  TicketTitle TEXT NOT NULL,
  FromStatus  TEXT NULL
);
`,
};
