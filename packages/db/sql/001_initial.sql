-- Schema-Version 1 des Boards. In JEDER Board-Datenbank einmal ausführen (AiHarness und AiHarness_Test).
-- Vorher im Abfragefenster die richtige Datenbank wählen.

CREATE SCHEMA board;
GO

CREATE TABLE board.SchemaVersion (
  Version   int          NOT NULL CONSTRAINT PK_SchemaVersion PRIMARY KEY,
  AppliedAt datetime2(3) NOT NULL CONSTRAINT DF_SchemaVersion_AppliedAt DEFAULT SYSUTCDATETIME()
);

CREATE TABLE board.Users (
  Id        uniqueidentifier NOT NULL CONSTRAINT PK_Users PRIMARY KEY,
  Name      nvarchar(100)    NOT NULL,
  CreatedAt datetime2(3)     NOT NULL CONSTRAINT DF_Users_CreatedAt DEFAULT SYSUTCDATETIME()
);

CREATE TABLE board.Projects (
  Id               int IDENTITY(1,1) NOT NULL CONSTRAINT PK_Projects PRIMARY KEY,
  [Key]            nvarchar(6)       NOT NULL CONSTRAINT UQ_Projects_Key UNIQUE,
  Name             nvarchar(200)     NOT NULL,
  NextTicketNumber int               NOT NULL CONSTRAINT DF_Projects_Next DEFAULT 1,
  CreatedAt        datetime2(3)      NOT NULL,
  CreatedBy        uniqueidentifier  NOT NULL CONSTRAINT FK_Projects_Users REFERENCES board.Users(Id)
);

CREATE TABLE board.Boards (
  Id        int IDENTITY(1,1) NOT NULL CONSTRAINT PK_Boards PRIMARY KEY,
  ProjectId int               NOT NULL CONSTRAINT FK_Boards_Projects REFERENCES board.Projects(Id),
  OwnerId   uniqueidentifier  NOT NULL CONSTRAINT FK_Boards_Users REFERENCES board.Users(Id),
  Name      nvarchar(200)     NOT NULL,
  RepoPath  nvarchar(500)     NULL,
  CreatedAt datetime2(3)      NOT NULL,
  RowVer    rowversion
);
CREATE INDEX IX_Boards_Project ON board.Boards(ProjectId);

CREATE TABLE board.Tickets (
  Id              nvarchar(20)     NOT NULL CONSTRAINT PK_Tickets PRIMARY KEY,
  ProjectId       int              NOT NULL CONSTRAINT FK_Tickets_Projects REFERENCES board.Projects(Id),
  BoardId         int              NOT NULL CONSTRAINT FK_Tickets_Boards REFERENCES board.Boards(Id),
  Number          int              NOT NULL,
  Title           nvarchar(400)    NOT NULL,
  Description     nvarchar(max)    NOT NULL CONSTRAINT DF_Tickets_Description DEFAULT N'',
  Status          varchar(20)      NOT NULL CONSTRAINT CK_Tickets_Status CHECK (Status IN ('backlog','in_planning','planned','in_progress','blocked','done','not_needed')),
  Priority        varchar(10)      NOT NULL CONSTRAINT CK_Tickets_Priority CHECK (Priority IN ('low','normal','high','urgent')),
  SortOrder       int              NOT NULL,
  Assignee        nvarchar(100)    NULL,
  NeedsPlanning   bit              NOT NULL,
  Acceptance      nvarchar(max)    NOT NULL CONSTRAINT DF_Tickets_Acceptance DEFAULT N'',
  [Plan]          nvarchar(max)    NOT NULL CONSTRAINT DF_Tickets_Plan DEFAULT N'',
  Handbook        nvarchar(max)    NOT NULL CONSTRAINT DF_Tickets_Handbook DEFAULT N'',
  Diagram         nvarchar(max)    NOT NULL CONSTRAINT DF_Tickets_Diagram DEFAULT N'',
  BlockedReason   nvarchar(500)    NULL,
  PlannedAt       datetime2(3)     NULL,
  PlannedBy       nvarchar(100)    NULL,
  PlanningClaimBy nvarchar(100)    NULL,
  PlanningClaimAt datetime2(3)     NULL,
  CreatedAt       datetime2(3)     NOT NULL,
  CreatedById     uniqueidentifier NOT NULL CONSTRAINT FK_Tickets_Users REFERENCES board.Users(Id),
  CreatedByName   nvarchar(100)    NOT NULL,
  UpdatedAt       datetime2(3)     NOT NULL,
  Rev             int              NOT NULL,
  RowVer          rowversion,
  CONSTRAINT UQ_Tickets_Number UNIQUE (ProjectId, Number)
);
CREATE INDEX IX_Tickets_Board ON board.Tickets(BoardId, Status);

CREATE TABLE board.TicketLabels (
  TicketId nvarchar(20)  NOT NULL CONSTRAINT FK_TicketLabels_Tickets REFERENCES board.Tickets(Id) ON DELETE CASCADE,
  Position int           NOT NULL,
  Label    nvarchar(100) NOT NULL,
  CONSTRAINT PK_TicketLabels PRIMARY KEY (TicketId, Label)
);

-- Zweiter Fremdschlüssel ohne CASCADE (SQL Server erlaubt keine zwei Kaskadenpfade);
-- der Dienst löscht Verweise auf ein Ticket vor dem Ticket selbst.
CREATE TABLE board.TicketDependencies (
  TicketId    nvarchar(20) NOT NULL CONSTRAINT FK_TicketDeps_Tickets REFERENCES board.Tickets(Id) ON DELETE CASCADE,
  Position    int          NOT NULL,
  DependsOnId nvarchar(20) NOT NULL CONSTRAINT FK_TicketDeps_DependsOn REFERENCES board.Tickets(Id),
  CONSTRAINT PK_TicketDependencies PRIMARY KEY (TicketId, DependsOnId)
);
CREATE INDEX IX_TicketDeps_DependsOn ON board.TicketDependencies(DependsOnId);

CREATE TABLE board.Comments (
  Id         bigint IDENTITY(1,1) NOT NULL CONSTRAINT PK_Comments PRIMARY KEY,
  TicketId   nvarchar(20)     NOT NULL CONSTRAINT FK_Comments_Tickets REFERENCES board.Tickets(Id) ON DELETE CASCADE,
  AuthorId   uniqueidentifier NOT NULL CONSTRAINT FK_Comments_Users REFERENCES board.Users(Id),
  AuthorName nvarchar(100)    NOT NULL,
  Body       nvarchar(max)    NOT NULL,
  CreatedAt  datetime2(3)     NOT NULL,
  RowVer     rowversion
);
CREATE INDEX IX_Comments_Ticket ON board.Comments(TicketId);

CREATE TABLE board.History (
  Id        bigint IDENTITY(1,1) NOT NULL CONSTRAINT PK_History PRIMARY KEY,
  TicketId  nvarchar(20)     NOT NULL CONSTRAINT FK_History_Tickets REFERENCES board.Tickets(Id) ON DELETE CASCADE,
  At        datetime2(3)     NOT NULL,
  ById      uniqueidentifier NOT NULL CONSTRAINT FK_History_Users REFERENCES board.Users(Id),
  ByName    nvarchar(100)    NOT NULL,
  Field     varchar(30)      NOT NULL,
  FromValue nvarchar(max)    NULL,
  ToValue   nvarchar(max)    NULL,
  Note      nvarchar(200)    NULL
);
CREATE INDEX IX_History_Ticket ON board.History(TicketId);

CREATE TABLE board.Notes (
  Id         int IDENTITY(1,1) NOT NULL CONSTRAINT PK_Notes PRIMARY KEY,
  BoardId    int              NOT NULL CONSTRAINT FK_Notes_Boards REFERENCES board.Boards(Id) ON DELETE CASCADE,
  AuthorId   uniqueidentifier NOT NULL CONSTRAINT FK_Notes_Users REFERENCES board.Users(Id),
  AuthorName nvarchar(100)    NOT NULL,
  Body       nvarchar(max)    NOT NULL,
  Source     nvarchar(20)     NULL,
  Labels     nvarchar(1000)   NOT NULL CONSTRAINT DF_Notes_Labels DEFAULT N'',
  State      varchar(10)      NOT NULL CONSTRAINT CK_Notes_State CHECK (State IN ('open','taken','dropped')),
  TicketId   nvarchar(20)     NULL,
  Reason     nvarchar(1000)   NULL,
  CreatedAt  datetime2(3)     NOT NULL,
  RowVer     rowversion
);
CREATE INDEX IX_Notes_Board ON board.Notes(BoardId, State);

CREATE TABLE board.Deletions (
  Id          bigint IDENTITY(1,1) NOT NULL CONSTRAINT PK_Deletions PRIMARY KEY,
  At          datetime2(3)     NOT NULL,
  ById        uniqueidentifier NOT NULL CONSTRAINT FK_Deletions_Users REFERENCES board.Users(Id),
  ByName      nvarchar(100)    NOT NULL,
  TicketId    nvarchar(20)     NOT NULL,
  TicketTitle nvarchar(400)    NOT NULL,
  FromStatus  varchar(20)      NULL
);

INSERT INTO board.SchemaVersion (Version) VALUES (1);
GO
