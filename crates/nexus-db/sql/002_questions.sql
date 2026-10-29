-- Schema-Version 2 des Boards: Fragen (Teilprojekt 3). In AiHarness_Test und in AiHarness ausführen, nach 001.
-- Vorher im Abfragefenster die richtige Datenbank wählen. Ein einziger Batch in einer Transaktion:
-- fehlt Version 1 oder ist Version 2 schon eingespielt, bricht das Skript ab und ändert nichts.

SET XACT_ABORT ON;
BEGIN TRANSACTION;

IF NOT EXISTS (SELECT 1 FROM board.SchemaVersion WHERE Version = 1)
  THROW 50001, N'Schema-Version 1 fehlt: zuerst 001_initial.sql ausführen.', 1;
IF EXISTS (SELECT 1 FROM board.SchemaVersion WHERE Version >= 2)
  THROW 50002, N'Schema-Version 2 ist bereits eingespielt.', 1;

ALTER TABLE board.Projects ADD NextQuestionNumber int NOT NULL CONSTRAINT DF_Projects_NextQuestion DEFAULT 1;

CREATE TABLE board.Questions (
  Id              nvarchar(20)     NOT NULL CONSTRAINT PK_Questions PRIMARY KEY,
  ProjectId       int              NOT NULL CONSTRAINT FK_Questions_Projects REFERENCES board.Projects(Id),
  Number          int              NOT NULL,
  Title           nvarchar(400)    NOT NULL,
  Description     nvarchar(max)    NOT NULL CONSTRAINT DF_Questions_Description DEFAULT N'',
  State           varchar(10)      NOT NULL CONSTRAINT CK_Questions_State CHECK (State IN ('open','answered','discarded')),
  Occasion        nvarchar(400)    NULL,
  OriginKind      varchar(10)      NOT NULL CONSTRAINT CK_Questions_OriginKind CHECK (OriginKind IN ('human','session','subagent')),
  OriginName      nvarchar(100)    NOT NULL,
  OriginSession   nvarchar(100)    NULL,
  DecisionOutcome varchar(10)      NULL CONSTRAINT CK_Questions_Outcome CHECK (DecisionOutcome IN ('answered','discarded')),
  DecisionOption  nvarchar(10)     NULL,
  DecisionText    nvarchar(max)    NULL,
  DecisionReason  nvarchar(max)    NULL,
  DecidedById     uniqueidentifier NULL CONSTRAINT FK_Questions_DecidedBy REFERENCES board.Users(Id),
  DecidedByName   nvarchar(100)    NULL,
  DecidedAt       datetime2(3)     NULL,
  CreatedAt       datetime2(3)     NOT NULL,
  CreatedById     uniqueidentifier NOT NULL CONSTRAINT FK_Questions_Users REFERENCES board.Users(Id),
  UpdatedAt       datetime2(3)     NOT NULL,
  Rev             int              NOT NULL,
  RowVer          rowversion,
  CONSTRAINT UQ_Questions_Number UNIQUE (ProjectId, Number)
);
CREATE INDEX IX_Questions_Project ON board.Questions(ProjectId, State);

CREATE TABLE board.QuestionTags (
  QuestionId nvarchar(20)  NOT NULL CONSTRAINT FK_QuestionTags_Questions REFERENCES board.Questions(Id) ON DELETE CASCADE,
  Position   int           NOT NULL,
  Tag        nvarchar(100) NOT NULL,
  CONSTRAINT PK_QuestionTags PRIMARY KEY (QuestionId, Tag)
);

-- Betroffene Tickets. Der Verweis aufs Ticket hat kein CASCADE: der Dienst entfernt den Bezug
-- vor dem Löschen des Tickets und schreibt dabei den Verlauf der Frage.
CREATE TABLE board.QuestionTickets (
  QuestionId nvarchar(20) NOT NULL CONSTRAINT FK_QuestionTickets_Questions REFERENCES board.Questions(Id) ON DELETE CASCADE,
  TicketId   nvarchar(20) NOT NULL CONSTRAINT FK_QuestionTickets_Tickets REFERENCES board.Tickets(Id),
  CONSTRAINT PK_QuestionTickets PRIMARY KEY (QuestionId, TicketId)
);
CREATE INDEX IX_QuestionTickets_Ticket ON board.QuestionTickets(TicketId);

-- Abhängigkeiten auf Fragen oder Tickets; ohne Fremdschlüssel aufs Ziel (zwei mögliche Tabellen)
CREATE TABLE board.QuestionDependencies (
  QuestionId  nvarchar(20) NOT NULL CONSTRAINT FK_QuestionDeps_Questions REFERENCES board.Questions(Id) ON DELETE CASCADE,
  Position    int          NOT NULL,
  DependsOnId nvarchar(20) NOT NULL,
  CONSTRAINT PK_QuestionDependencies PRIMARY KEY (QuestionId, DependsOnId)
);
CREATE INDEX IX_QuestionDeps_DependsOn ON board.QuestionDependencies(DependsOnId);

CREATE TABLE board.QuestionResearch (
  Id         bigint IDENTITY(1,1) NOT NULL CONSTRAINT PK_QuestionResearch PRIMARY KEY,
  QuestionId nvarchar(20)     NOT NULL CONSTRAINT FK_QuestionResearch_Questions REFERENCES board.Questions(Id) ON DELETE CASCADE,
  At         datetime2(3)     NOT NULL,
  ById       uniqueidentifier NOT NULL CONSTRAINT FK_QuestionResearch_Users REFERENCES board.Users(Id),
  ByName     nvarchar(100)    NOT NULL,
  Kind       varchar(10)      NOT NULL CONSTRAINT CK_QuestionResearch_Kind CHECK (Kind IN ('human','session','subagent')),
  Context    nvarchar(max)    NOT NULL CONSTRAINT DF_QuestionResearch_Context DEFAULT N''
);
CREATE INDEX IX_QuestionResearch_Question ON board.QuestionResearch(QuestionId);

-- Optionen O1, O2 … fortlaufend je Frage; Vor- und Nachteile als JSON-Text.
-- Nur der Verweis auf die Recherche kaskadiert (SQL Server erlaubt keine zwei Kaskadenpfade).
CREATE TABLE board.QuestionOptions (
  QuestionId  nvarchar(20)  NOT NULL,
  ResearchId  bigint        NOT NULL CONSTRAINT FK_QuestionOptions_Research REFERENCES board.QuestionResearch(Id) ON DELETE CASCADE,
  OptionId    varchar(10)   NOT NULL,
  Title       nvarchar(400) NOT NULL,
  Text        nvarchar(max) NOT NULL,
  Pros        nvarchar(max) NOT NULL CONSTRAINT DF_QuestionOptions_Pros DEFAULT N'[]',
  Cons        nvarchar(max) NOT NULL CONSTRAINT DF_QuestionOptions_Cons DEFAULT N'[]',
  Recommended bit           NOT NULL,
  CONSTRAINT PK_QuestionOptions PRIMARY KEY (QuestionId, OptionId)
);
CREATE INDEX IX_QuestionOptions_Research ON board.QuestionOptions(ResearchId);

CREATE TABLE board.QuestionComments (
  Id         bigint IDENTITY(1,1) NOT NULL CONSTRAINT PK_QuestionComments PRIMARY KEY,
  QuestionId nvarchar(20)     NOT NULL CONSTRAINT FK_QuestionComments_Questions REFERENCES board.Questions(Id) ON DELETE CASCADE,
  AuthorId   uniqueidentifier NOT NULL CONSTRAINT FK_QuestionComments_Users REFERENCES board.Users(Id),
  AuthorName nvarchar(100)    NOT NULL,
  AuthorKind varchar(10)      NOT NULL CONSTRAINT CK_QuestionComments_Kind CHECK (AuthorKind IN ('human','session','subagent')),
  Body       nvarchar(max)    NOT NULL,
  CreatedAt  datetime2(3)     NOT NULL
);
CREATE INDEX IX_QuestionComments_Question ON board.QuestionComments(QuestionId);

CREATE TABLE board.QuestionHistory (
  Id         bigint IDENTITY(1,1) NOT NULL CONSTRAINT PK_QuestionHistory PRIMARY KEY,
  QuestionId nvarchar(20)     NOT NULL CONSTRAINT FK_QuestionHistory_Questions REFERENCES board.Questions(Id) ON DELETE CASCADE,
  At         datetime2(3)     NOT NULL,
  ById       uniqueidentifier NOT NULL CONSTRAINT FK_QuestionHistory_Users REFERENCES board.Users(Id),
  ByName     nvarchar(100)    NOT NULL,
  ByKind     varchar(10)      NULL CONSTRAINT CK_QuestionHistory_Kind CHECK (ByKind IN ('human','session','subagent')),
  Type       varchar(20)      NOT NULL,
  Text       nvarchar(max)    NOT NULL
);
CREATE INDEX IX_QuestionHistory_Question ON board.QuestionHistory(QuestionId);

INSERT INTO board.SchemaVersion (Version) VALUES (2);
COMMIT TRANSACTION;
GO
